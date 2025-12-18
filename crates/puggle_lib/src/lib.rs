use std::{
    collections::HashMap,
    ffi::OsStr,
    fs::File,
    path::{Path, PathBuf},
};

use minijinja::{Environment, State, Value, value::Kwargs};
use pulldown_cmark::{
    CodeBlockKind, CowStr, Event, HeadingLevel, MetadataBlockKind, Parser, Tag, TagEnd,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use time::OffsetDateTime;
use url::Url;

#[derive(Clone, Debug, Deserialize)]
pub struct Config {
    pub pages: Vec<Page>,
    pub templates_dir: PathBuf,
    pub dest_dir: PathBuf,
    pub base_url: Url,
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct PageWithEntries {
    name: String,
    description: Option<String>,
    #[serde(default)]
    rss: bool,
    rss_name: Option<String>,
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
    WithEntries(PageWithEntries),
    Standalone(StandalonePage),
}

impl Page {
    fn get_template_path(&self) -> &Path {
        match self {
            Page::WithEntries(PageWithEntries { template_path, .. }) => template_path.as_path(),
            Page::Standalone(StandalonePage { template_path, .. }) => template_path.as_path(),
        }
    }

    fn get_name(&self) -> &str {
        match self {
            Page::WithEntries(PageWithEntries { name, .. }) => name.as_str(),
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
    pub fn from_file() -> Result<Self, config::ConfigError> {
        let conf = config::Config::builder()
            .add_source(config::File::with_name("puggle.yaml").required(false))
            .add_source(config::File::with_name("puggle.yml").required(false))
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
    pub author_email: Option<String>,
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

#[derive(Debug, Error)]
pub enum ExtractMetadataError {
    #[error("failed to deserialize file \"{0}\" metadata. reason: {1}")]
    Deserialize(PathBuf, serde_yaml::Error),
}

pub struct PuggleParser<'a> {
    pub metadata: Option<Metadata>,
    pub events: Vec<Event<'a>>,
}

#[derive(Clone)]
pub struct RssFeed<'a> {
    pub name: Option<&'a String>,
    pub description: Option<String>,
    pub items: Vec<rss::Item>,
}

pub fn parse<'a>(
    config: Config,
    parser: Parser<'a>,
    page_path: String,
) -> color_eyre::Result<PuggleParser<'a>> {
    let mut metadata = None;
    let mut record_metadata = false;
    let mut record_code_block = false;
    let mut record_folded_code_block = false;
    let mut record_heading = false;
    let mut record_folded_code_block_summary = false;
    let mut new_events = Vec::new();
    // let syntax_set = two_face::syntax::extra_newlines();
    // let mut syntax = syntax_set.find_syntax_plain_text();
    // let theme_set = two_face::theme::extra();
    // let theme = theme_set.get(two_face::theme::EmbeddedThemeName::DarkNeon);
    let mut codeblock = String::new();
    let mut heading_text = String::new();
    let mut detected_lang: Option<&str> = None;
    // let mut prev_folded_line: Option<&str> = None;

    for event in parser {
        match event {
            Event::Start(Tag::MetadataBlock(MetadataBlockKind::YamlStyle)) => {
                record_metadata = true;
                new_events.push(event);
            }
            Event::End(TagEnd::MetadataBlock(MetadataBlockKind::YamlStyle)) => {
                record_metadata = false;
                new_events.push(event);
            }
            Event::Code(CowStr::Borrowed(txt)) => {
                if record_heading {
                    // let code = format!("<code>{txt}</code>");
                    heading_text.push_str(txt);
                } else {
                    new_events.push(event);
                }
            }
            Event::Text(CowStr::Borrowed(txt)) => {
                if record_metadata {
                    metadata = Some(txt.to_string());
                }

                // FIXME: Good golly I have to clean this up
                if record_code_block {
                    codeblock.push_str("<pre><code>");

                    for ref mut line in txt.split("\n") {
                        if line.starts_with("### FOLD_START") {
                            record_folded_code_block = true;
                            record_folded_code_block_summary = true;
                            codeblock.push_str("<details><summary class=\"foldable\">");
                            continue;
                        }

                        if line.starts_with("### FOLD_END") {
                            codeblock.push_str("</details>");
                            // codeblock.push_str("<span>");
                            // prev_folded_line.map(|line| codeblock.push_str());
                            // codeblock.push_str("</span>");
                            continue;
                        }

                        if record_folded_code_block && record_folded_code_block_summary {
                            if let Some(stripped_line) = line.strip_prefix(" ") {
                                *line = stripped_line;
                            }
                            codeblock.push_str(html_escape::encode_text(line).as_ref());
                            codeblock.push_str("</summary>");
                            record_folded_code_block_summary = false;
                            continue;
                        }

                        match (line.get(0..1), detected_lang) {
                            (Some("+"), Some("diff")) => {
                                codeblock
                                    .push_str("<span style=\"background: green; color: white;\">");
                                codeblock.push_str(html_escape::encode_text(line).as_ref());
                                codeblock.push_str("</span>\n");
                            }
                            (Some("-"), Some("diff")) => {
                                codeblock
                                    .push_str("<span style=\"background: red; color: white;\">");
                                codeblock.push_str(html_escape::encode_text(line).as_ref());
                                codeblock.push_str("</span>\n");
                            }
                            _ => {
                                codeblock.push_str("<span>");
                                codeblock.push_str(html_escape::encode_text(line).as_ref());
                                codeblock.push_str("</span>\n");
                                // if record_folded_code_block {
                                //     prev_folded_line = Some(line);
                                // }
                            }
                        }
                    }
                    codeblock = codeblock.trim_end_matches("<span></span>\n").to_string();
                    codeblock.push_str("</code></pre>");
                }

                if record_heading {
                    heading_text.push_str(txt);
                }

                if !record_code_block && !record_heading {
                    new_events.push(event);
                }
            }
            Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(CowStr::Borrowed(lang)))) => {
                detected_lang = Some(lang);
                record_code_block = true;
            }
            Event::End(TagEnd::CodeBlock) => {
                let html_event = Event::Html(CowStr::from(codeblock.clone()));
                new_events.push(html_event);
                codeblock.clear();
                record_code_block = false;
            }
            Event::Start(Tag::Heading { .. }) => {
                record_heading = true;
            }
            Event::End(TagEnd::Heading(pulldown_cmark::HeadingLevel::H1)) => {
                let heading = format!("<h1>{heading_text}</h1>",);
                let html_event = Event::Html(CowStr::from(heading));
                new_events.push(html_event);
                heading_text.clear();
                record_heading = false;
            }
            Event::End(TagEnd::Heading(heading_level)) => {
                let slug = heading_text.replace(" ", "-").to_lowercase();
                let slug = slug.trim();
                // FIXME: bruh
                let mut heading_url = config
                    .base_url
                    .join(page_path.as_str())
                    .expect("unable to construct heading URL");

                heading_url.set_fragment(Some(slug));

                let heading = format!(
                    "<{heading_level} id=\"{slug}\"><a href=\"{heading_url}\">{heading_text}</a></{heading_level}>"
                );
                let html_event = Event::Html(CowStr::from(heading));
                new_events.push(html_event);
                heading_text.clear();
                record_heading = false;
            }
            e => {
                new_events.push(e);
            }
        }
    }

    let metadata = if let Some(metadata) = metadata {
        let metadata: Metadata = serde_yaml::from_str(metadata.as_str())?;

        let metadata = Metadata {
            unix_created_at: metadata.created_at.map(|dt| dt.unix_timestamp()),
            unix_updated_at: metadata.updated_at.map(|dt| dt.unix_timestamp()),
            ..metadata
        };

        Some(metadata)
    } else {
        None
    };

    let pp = PuggleParser {
        metadata,
        events: new_events,
    };
    Ok(pp)
}

fn render_partial(
    inner: String,
    metadata: &Metadata,
    template_handle: &TemplateHandle,
) -> Result<String, minijinja::Error> {
    let html = template_handle
        .env
        .template_from_str(inner.as_str())?
        .render(minijinja::context!(metadata => metadata))?;

    Ok(html)
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
    println!("Config: {:#?}", config);
    let template_handle = TemplateHandle::new(config.templates_dir.as_path());
    let mut cmark_opts = pulldown_cmark::Options::empty();

    cmark_opts.insert(pulldown_cmark::Options::ENABLE_FOOTNOTES);
    cmark_opts.insert(pulldown_cmark::Options::ENABLE_STRIKETHROUGH);
    cmark_opts.insert(pulldown_cmark::Options::ENABLE_TASKLISTS);
    cmark_opts.insert(pulldown_cmark::Options::ENABLE_SMART_PUNCTUATION);
    cmark_opts.insert(pulldown_cmark::Options::ENABLE_YAML_STYLE_METADATA_BLOCKS);
    cmark_opts.insert(pulldown_cmark::Options::ENABLE_MATH);
    cmark_opts.insert(pulldown_cmark::Options::ENABLE_GFM);
    cmark_opts.insert(pulldown_cmark::Options::ENABLE_SUPERSCRIPT);
    cmark_opts.insert(pulldown_cmark::Options::ENABLE_SUBSCRIPT);
    cmark_opts.insert(pulldown_cmark::Options::ENABLE_WIKILINKS);

    let mut context: HashMap<&str, Vec<Metadata>> = HashMap::new();
    let mut feed_context: HashMap<&str, RssFeed> = HashMap::new();

    let pages_with_entries: Vec<&PageWithEntries> =
        config.pages.iter().fold(Vec::new(), |mut acc, page| {
            if let Page::WithEntries(page) = page {
                acc.push(page);
                acc
            } else {
                acc
            }
        });

    for page in pages_with_entries {
        let mut metadata_list = Vec::new();
        let mut rss_items: Vec<rss::Item> = Vec::new();

        for entry in page.entries.iter() {
            match entry {
                Entry::Dir {
                    source_dir,
                    template_path,
                } => {
                    let files = get_markdown_paths(source_dir.as_path())?;

                    for file in files {
                        let markdown = std::fs::read_to_string(file.as_path())?;
                        let parser = Parser::new_ext(markdown.as_str(), cmark_opts);
                        let md_file_name = file.file_stem().ok_or(ParseFilesError::FileName)?;

                        println!(
                            "Page path: {}",
                            format!("{}/{}", page.name, md_file_name.to_str().unwrap())
                        );

                        let pp = parse(
                            config.clone(),
                            parser,
                            format!("{}/{}", page.name, md_file_name.to_str().unwrap()),
                        )?;

                        let mut html_partial = String::new();

                        pulldown_cmark::html::push_html(&mut html_partial, pp.events.into_iter());

                        let metadata = pp
                            .metadata
                            .map(|metadata| Metadata {
                                file_name: md_file_name.to_string_lossy().to_string(),
                                ..metadata
                            })
                            .ok_or(color_eyre::Report::msg(format!(
                                "failed to extract metadata from file {:?}",
                                file.as_path()
                            )))?;

                        let html = render_entry(
                            html_partial.clone(),
                            &metadata,
                            template_path.as_path(),
                            &template_handle,
                        )?;

                        if page.rss {
                            let item = generate_rss_item(
                                &template_handle,
                                &config,
                                metadata.clone(),
                                html_partial.clone(),
                            )
                            .unwrap();
                            rss_items.push(item);
                        }

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
                    let md_file_name =
                        markdown_path.file_stem().ok_or(ParseFilesError::FileName)?;
                    let markdown = std::fs::read_to_string(markdown_path.as_path())?;
                    let parser = Parser::new_ext(markdown.as_str(), cmark_opts);
                    let pp = parse(
                        config.clone(),
                        parser,
                        format!("{}/{}", page.name, md_file_name.to_str().unwrap()),
                    )?;
                    let mut html_partial = String::new();

                    pulldown_cmark::html::push_html(&mut html_partial, pp.events.into_iter());

                    let metadata = pp
                        .metadata
                        .map(|metadata| Metadata {
                            file_name: md_file_name.to_string_lossy().to_string(),
                            ..metadata
                        })
                        .unwrap();

                    let html = render_entry(
                        html_partial.clone(),
                        &metadata,
                        template_path.as_path(),
                        &template_handle,
                    )?;

                    if page.rss {
                        let item = generate_rss_item(
                            &template_handle,
                            &config,
                            metadata.clone(),
                            html_partial.clone(),
                        )
                        .unwrap();
                        rss_items.push(item);
                    }

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

            match feed_context.get_mut(page.name.as_str()) {
                Some(feed) => feed.items.append(&mut rss_items),
                None => {
                    feed_context.insert(
                        page.name.as_str(),
                        RssFeed {
                            name: page.rss_name.as_ref(),
                            description: page.description.clone(),
                            items: rss_items.clone(),
                        },
                    );
                }
            }
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

    // Write RSS feeds
    for (page_name, rss_feed) in feed_context.into_iter() {
        // Create RSS feed
        let channel = rss::ChannelBuilder::default()
            .title(
                rss_feed
                    .name
                    .map_or(page_name, |feed_name| feed_name.as_str()),
            )
            .link(config.base_url.to_string())
            .description(rss_feed.description.unwrap_or("".to_string()))
            .items(rss_feed.items.clone())
            .language("en".to_string())
            .atom_ext(Some(rss::extension::atom::AtomExtension {
                links: vec![rss::extension::atom::Link {
                    rel: "self".into(),
                    href: config.base_url.to_string(),
                    ..Default::default()
                }],
            }))
            .build();

        let target_dir = PathBuf::from(config.dest_dir.as_os_str())
            .join(page_name)
            .with_extension("rss");

        let mut rss_buffer = File::create(target_dir).unwrap();
        channel.write_to(&mut rss_buffer).unwrap();
        // channel.validate().unwrap();
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

fn generate_rss_item(
    template_handle: &TemplateHandle,
    config: &Config,
    metadata: Metadata,
    html: String,
) -> Result<rss::Item, minijinja::Error> {
    let page_url = config
        .base_url
        .join(metadata.file_name.as_str())
        .expect("failed to join file name with base URL");

    let guid = rss::GuidBuilder::default()
        .value(page_url.to_string())
        .permalink(false)
        .build();

    let rendered_html_partial = render_partial(html, &metadata, &template_handle).unwrap();

    let item = rss::ItemBuilder::default()
        .title(metadata.title.clone())
        .author(metadata.author_email.clone())
        .content(rendered_html_partial)
        .description(metadata.summary.clone())
        .link(page_url.to_string())
        .pub_date(metadata.created_at.map(|ts| {
            ts.format(&time::format_description::well_known::Rfc2822)
                .unwrap()
        }))
        .guid(guid)
        .build();

    Ok(item)
}
