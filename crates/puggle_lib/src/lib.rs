use std::{
    collections::HashMap,
    ffi::OsStr,
    path::{Path, PathBuf},
};

use minijinja::{value::Kwargs, Environment, State, Value};
use pulldown_cmark::{Event, MetadataBlockKind, Parser, Tag, TagEnd};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use time::OffsetDateTime;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub pages: Vec<Page>,
    pub templates_dir: PathBuf,
    pub dest_dir: PathBuf,
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct PageEntries {
    name: String,
    template_path: PathBuf,
    entries: Vec<Entry>,
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct StandalonePage {
    name: String,
    template_path: PathBuf,
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(untagged)]
pub enum Page {
    WithEntries(PageEntries),
    Standalone(StandalonePage),
}

impl Page {
    fn get_template_path(&self) -> &Path {
        match self {
            Page::WithEntries(PageEntries { template_path, .. }) => template_path.as_path(),
            Page::Standalone(StandalonePage { template_path, .. }) => template_path.as_path(),
        }
    }

    fn get_name(&self) -> &str {
        match self {
            Page::WithEntries(PageEntries { name, .. }) => name.as_str(),
            Page::Standalone(StandalonePage { name, .. }) => name.as_str(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(untagged)]
enum Entry {
    Dir {
        source_dir: PathBuf,
        template_path: PathBuf,
    },
    File {
        markdown_path: PathBuf,
        template_path: PathBuf,
    },
}

impl Config {
    pub fn from_file(path: &str) -> Result<Self, config::ConfigError> {
        let conf = config::Config::builder()
            .add_source(config::File::with_name(path).required(false))
            .build()?;

        conf.try_deserialize()
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Metadata {
    pub title: String,
    #[serde(with = "time::serde::rfc3339::option")]
    pub created_at: Option<OffsetDateTime>,
    #[serde(with = "time::serde::rfc3339::option")]
    pub updated_at: Option<OffsetDateTime>,
    #[serde(skip_deserializing)]
    pub unix_created_at: Option<i64>,
    #[serde(skip_deserializing)]
    pub unix_updated_at: Option<i64>,
    pub tags: Vec<String>,
    #[serde(skip_deserializing)]
    pub file_name: String,
    pub cover: Option<String>,
    pub summary: Option<String>,
    pub aliases: Option<Vec<PathBuf>>,
    pub custom: Option<HashMap<String, String>>,
}

pub struct TemplateHandle {
    env: Environment<'static>,
}

impl TemplateHandle {
    pub fn new(templates_dir: &Path) -> Self {
        let mut env = minijinja::Environment::new();
        env.set_loader(minijinja::path_loader(templates_dir));
        env.add_filter("published_on", published_on);
        minijinja_contrib::add_to_environment(&mut env);

        Self { env }
    }
}

#[derive(Debug, Error)]
pub enum ParseFilesError {
    #[error("")]
    Io(#[from] std::io::Error),
    #[error("")]
    Parent,
    #[error("")]
    FileName,
    #[error("")]
    Metadata(#[from] ExtractMetadataError),
    #[error("failed to load template. reason: {0}")]
    TemplateEnvironment(minijinja::Error),
    #[error("failed to render template. reason: {0}")]
    TemplateRender(minijinja::Error),
}

pub fn parse(input: &str, opts: pulldown_cmark::Options) -> String {
    let parser = Parser::new_ext(input, opts);
    let mut html = String::new();

    pulldown_cmark::html::push_html(&mut html, parser.into_iter());
    html
}

#[derive(Debug, Error)]
pub enum ExtractMetadataError {
    #[error("failed to deserialize file \"{0}\" metadata. reason: {1}")]
    Deserialize(PathBuf, serde_yml::Error),
}

pub fn extract_metadata(parser: Parser) -> color_eyre::Result<Option<Metadata>> {
    let mut metadata = None;
    let mut record = false;

    for event in parser {
        if let Event::Start(Tag::MetadataBlock(_)) = event {
            record = true;
        }

        match event {
            Event::Start(Tag::MetadataBlock(MetadataBlockKind::YamlStyle)) => {
                record = true;
            }
            Event::End(TagEnd::MetadataBlock(MetadataBlockKind::YamlStyle)) => {
                break;
            }
            Event::Text(txt) => {
                if record {
                    metadata = Some(txt.to_string());
                }
            }
            _ => {
                break;
            }
        }
    }

    if let Some(metadata) = metadata {
        let metadata: Metadata = serde_yml::from_str(metadata.as_str())?;

        let metadata = Metadata {
            unix_created_at: metadata.created_at.map(|dt| dt.unix_timestamp()),
            unix_updated_at: metadata.updated_at.map(|dt| dt.unix_timestamp()),
            ..metadata
        };

        Ok(Some(metadata))
    } else {
        Ok(None)
    }
}

fn render_entry(
    inner: String,
    metadata: &Metadata,
    template_path: &Path,
    template_handle: &TemplateHandle,
) -> Result<String, minijinja::Error> {
    let template = [
        format!("{{% extends \"{}\" %}}", template_path.to_string_lossy()),
        "{% block content %}".to_string(),
        inner,
        "{% endblock %}".to_string(),
    ]
    .join("\n");

    let html = template_handle
        .env
        .template_from_str(template.as_str())?
        .render(minijinja::context!(metadata => metadata))?;

    Ok(html)
}

fn get_markdown_paths(dir: &Path) -> color_eyre::Result<Vec<PathBuf>> {
    println!("{:?}", dir);
    let paths = std::fs::read_dir(dir)?
        .filter(|entry| {
            if let Ok(entry) = entry {
                let path = entry.path();
                path.is_file() && path.extension().unwrap_or(OsStr::new("")) == "md"
            } else {
                false
            }
        })
        .map(|entry| entry.and_then(|entry| Ok(entry.path())))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(paths)
}

pub fn build_from_dir(config: Config) -> color_eyre::Result<()> {
    let template_handle = TemplateHandle::new(config.templates_dir.as_path());
    let cmark_opts = pulldown_cmark::Options::all();
    let mut context: HashMap<&str, Vec<Metadata>> = HashMap::new();

    let pages_with_entries: Vec<&PageEntries> =
        config.pages.iter().fold(Vec::new(), |mut acc, page| {
            if let Page::WithEntries(page) = page {
                acc.push(page);
                acc
            } else {
                acc
            }
        });

    for page in pages_with_entries {
        let mut metadata_list = vec![];

        for entry in page.entries.iter() {
            match entry {
                Entry::Dir {
                    source_dir,
                    template_path,
                } => {
                    let files = get_markdown_paths(source_dir.as_path())?;

                    for file in files {
                        let markdown = std::fs::read_to_string(file.as_path())?;
                        let html_partial = parse(markdown.as_str(), cmark_opts);
                        let parser = Parser::new_ext(markdown.as_str(), cmark_opts);
                        let metadata = extract_metadata(parser)?;

                        let md_file_name = file
                            .as_path()
                            .file_stem()
                            .ok_or(ParseFilesError::FileName)?;

                        let metadata = metadata
                            .map(|metadata| Metadata {
                                file_name: md_file_name.to_string_lossy().to_string(),
                                ..metadata
                            })
                            .ok_or(color_eyre::Report::msg(format!(
                                "failed to extract metadata from file {:?}",
                                file.as_path()
                            )))?;

                        let html = render_entry(
                            html_partial,
                            &metadata,
                            template_path.as_path(),
                            &template_handle,
                        )?;

                        // Write to file
                        let target_file = PathBuf::from(config.dest_dir.as_os_str())
                            .join(page.name.as_str())
                            .join(md_file_name)
                            .join("index")
                            .with_extension("html");

                        if !target_file
                            .parent()
                            .ok_or(ParseFilesError::Parent)?
                            .exists()
                        {
                            std::fs::create_dir_all(
                                target_file.parent().ok_or(ParseFilesError::Parent)?,
                            )?;
                        }

                        std::fs::write(target_file, html)?;

                        if let Some(ref aliases) = metadata.aliases {
                            for alias in aliases {
                                let alias_file = config
                                    .dest_dir
                                    .join(page.name.as_str())
                                    .join(alias)
                                    .join("index")
                                    .with_extension("html");

                                if !alias_file.parent().ok_or(ParseFilesError::Parent)?.exists() {
                                    std::fs::create_dir_all(
                                        alias_file.parent().ok_or(ParseFilesError::Parent)?,
                                    )?;
                                }

                                let redir_html = format!(
                                    "<!DOCTYPE html>
<html>
  <head>
    <title>{0}</title>
    <link rel=\"canonical\" href=\"/{1}\"/>
    <meta http-equiv=\"content-type\" content=\"text/html; charset=utf-8\"/>
    <meta http-equiv=\"refresh\" content=\"0; url=/{1}\"/>
  </head>
  <body>
    If you aren't redirected, you can manually click this link:
    <a href=\"/{1}\">/{1}</a>.
  </body>
</html>",
                                    metadata.title,
                                    PathBuf::from(page.name.as_str())
                                        .join(md_file_name)
                                        .display(),
                                );

                                std::fs::write(alias_file.as_path(), redir_html)?;
                            }
                        }

                        metadata_list.push(metadata);
                    }
                }
                Entry::File {
                    markdown_path,
                    template_path,
                } => {
                    let markdown = std::fs::read_to_string(markdown_path.as_path())?;
                    let html_partial = parse(markdown.as_str(), cmark_opts);
                    let parser = Parser::new_ext(markdown.as_str(), cmark_opts);
                    let metadata = extract_metadata(parser)?;

                    let md_file_name =
                        markdown_path.file_stem().ok_or(ParseFilesError::FileName)?;

                    let metadata = metadata
                        .map(|metadata| Metadata {
                            file_name: md_file_name.to_string_lossy().to_string(),
                            ..metadata
                        })
                        .unwrap();

                    let html = render_entry(
                        html_partial,
                        &metadata,
                        template_path.as_path(),
                        &template_handle,
                    )?;

                    // Write to file
                    let target_file = PathBuf::from(config.dest_dir.as_os_str())
                        .join(page.name.as_str())
                        .join(md_file_name)
                        .join("index")
                        .with_extension("html");

                    if !target_file
                        .parent()
                        .ok_or(ParseFilesError::Parent)?
                        .exists()
                    {
                        std::fs::create_dir_all(
                            target_file.parent().ok_or(ParseFilesError::Parent)?,
                        )?;
                    }

                    std::fs::write(target_file, html)?;

                    metadata_list.push(metadata);
                }
            }

            context.insert(page.name.as_str(), metadata_list.clone());
        }
    }

    // Render standalone pages
    for page in config.pages.iter() {
        let template_path = page
            .get_template_path()
            .to_str()
            .ok_or(color_eyre::Report::msg(
                "page template path is not a valid unicode",
            ))?;

        let html = template_handle
            .env
            .get_template(template_path)
            .map_err(|e| ParseFilesError::TemplateEnvironment(e))?
            .render(minijinja::context!(pages => context))
            .map_err(|e| ParseFilesError::TemplateRender(e))?;

        let target_file = PathBuf::from(config.dest_dir.as_path())
            .join(page.get_name())
            .join("index")
            .with_extension("html");

        if !target_file
            .parent()
            .ok_or(ParseFilesError::Parent)?
            .exists()
        {
            std::fs::create_dir_all(target_file.parent().ok_or(ParseFilesError::Parent)?)?;
        }

        let _ = std::fs::write(target_file, html);
    }

    Ok(())
}

fn published_on(state: &State, value: Value, kwargs: Kwargs) -> Result<String, minijinja::Error> {
    let user_date_str = minijinja_contrib::filters::datetimeformat(state, value.clone(), kwargs)?;

    let date_str = minijinja_contrib::filters::datetimeformat(
        state,
        value,
        Kwargs::from_iter([("format", Value::from("iso"))]),
    )?;
    Ok(format!(
        "Published on <time datetime=\"{}\">{} UTC</time>",
        date_str, user_date_str
    ))
}
