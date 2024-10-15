# puggle

![image](./cover.png)

A simple static site generator for my personal use.

> [!CAUTION]
> Is the code in good shape? No. Should you use it for your own stuff? Probably not,
> but you can. At the moment this is experimental, and things could break at any
> time without proper versioning or notice.

(Anteater from Im australischen Busch und an den Küsten des Korallenmeeres. Reiseerlebnisse und Beobachtungen eines Naturforschers in Australien, Neu Guinea und den Molukken (1866) -
<a href="https://creazilla.com/media/traditional-art/3446271/anteater-from-im-australischen-busch-und-an-den-kusten-des-korallenmeeres.-reiseerlebnisse-und-beobachtungen-eines-naturforschers-in-australien-neu-guinea-und-den-molukken-1866-published-by-richard-wolfgang-semon.">Source</a>)

## Examples

- [github.com/sekunho/sekun.net](https://github.com/sekunho/sekun.net)

## Quick start

A `puggle` project starts with a configuration file `puggle.yml`. This config
file allows us to define the pages we want to have for our static site.

```yaml
# ./puggle.yml
templates_dir: templates
dest_dir: dist

pages:
  - name: blog
    template_path: layout/blog.html
```

Here we defined a page called `blog`, as well as the relative path to the template
that it should use. This template will be used to generate the HTML file for our
blog page.

We also specified our templates directory. This is the base path of all
`template_path`s. So in our example, the actual relative path `puggle` will use
is `./templates/layout/blog.html`.

Next, let's create our blog page's template.

```html
<!-- ./templates/layout/blog.html -->
<!DOCTYPE html>
<html>
  <head>
    <title>Blog</title>
  </head>

  <body>
    Welcome to my blog.
  </body>
</html>
```

Running `puggle build` would create the following:

```
dist
└─ blog
   └─ index.html
```

...with an `dist/blog/index.html`

```html
<!-- ./dist/blog/index.html -->
<!DOCTYPE html>
<html>
  <head>
    <title>Blog</title>
  </head>

  <body>Welcome to my blog.</body>
</html>
```

This is not very interesting though because we want to have posts in our blog.
We also want our template to be slightly different from our main blog page.
So let's create some entries!

`puggle` has two ways of sourcing page entries:

1. Fetch every single markdown file in a specific directory, recursively; and
2. Referencing the markdown file directly.

Let's update our `puggle.yml`.

```yaml
# ./puggle.yml
templates_dir: templates
dest_dir: dist

pages:
  - name: blog
    template_path: layout/blog.html

    entries:
      - source_dir: blog/posts
        template_path: layout/post.html
```

Here we're defining our source directory for our blog's entries to be `./blog/posts`.
`puggle` will search for every single markdown file under that directory.

Then create `./blog/posts/first.md`

```md
<!-- ./blog/posts/first.md -->
---
title: First post
summary: For my first blog post, I shall...
cover: "/assets/images/post_cover.jpg"
created_at: 2024-06-29T17:29:00Z
updated_at:
tags: ["hello", "world"]
---

# First post

Hello, world!
```

> 💡 You'll notice a YAML-style metadata block at the top of the markdown file,
> and this is what allows you to do some pretty cool things with `puggle`. For
> now, just think of it as potentially useful data that we could use.
>
> Here's a quick rundown:
>
> - `title` (required): Used to label your page entry's title. You can use this to
> index a page's entries.
> - `created_at` (required): UTC timestamp of when the page was created. e.g `2024-06-29T17:29:00Z`
> - `updated_at` (can be left blank): UTC timestamp of when the page was updated. e.g `2024-06-29T17:29:00Z`
> - `tags` (required): A list of strings. You may define this as an empty list. e.g `["nixos", "rust"]`

And a template for our blog's entries

```html
<!-- ./templates/layout/post.html -->
<!DOCTYPE html>
<html>
  <head>
    <title>Blog Post</title>
  </head>

  <body>
    {% raw %}{% block content %}{% endblock %}{% endraw %}
  </body>
</html>
```

The `{% raw %}{% block content %}{% endblock %}{% endraw %}` defines a `block`
statement for us to inject content into it. `puggle` requires the content block
to be present in the entry template otherwise it would have nowhere to inject
the generated HTML file into, and would result in a blank HTML file.

`puggle build` would then create the following:

```
dist
└─ blog
   ├─ first
   │  └─ index.html
   └─ index.html
```

...with an `dist/blog/first/index.html`

```html
<!-- ./dist/blog/first/index.html -->
<!DOCTYPE html>
<html>
  <head>
    <title>Blog</title>
  </head>

  <body>
    <h1>First post</h1>
    <p>Hello, world!</p>
  </body>
</html>
```

### Page entry metadata

Metadata provides a lot of flexibility for you to inject data into your templates,
or markdown files.

Therefore changing our blog entry to

```md
<!-- ./blog/posts/first.md -->
---
title: First post
summary: For my first blog post, I shall...
cover: "/assets/images/post_cover.jpg"
created_at: 2024-06-29T17:29:00Z
updated_at:
tags: ["hello", "world"]
---

# {{ metadata.title }}

Hello, world!
```

Would give us the same result because `puggle` understands that we're referencing
the `title` attribute defined in the entry's metadata. `puggle` also allows you
to use _all_ page entries' metadata in other pages.

The entry's template file can also reference the entry's metadata so you could
set the `title` tag to our entry title for example.

```html
<!-- ./templates/layout/post.html -->
<!DOCTYPE html>
<html>
  <head>
    <title>{{ metadata.title }} - Blog</title>
  </head>

  <body>
    {% raw %}{% block content %}{% endblock %}{% endraw %}
  </body>
</html>
```

> You can't reference metadata attributes in a page entry from another entry
> because each entry can only know about its own metadata.

Let's index our blog's entries in our blog page!

```html
<!-- ./templates/layout/blog.html -->
<!DOCTYPE html>
<html>
  <head>
    <title>Blog</title>
  </head>

  <body>
    Welcome to my blog.

    {% raw %}{% for page_name, page_entries in sections|items %}
      {% if page_name == "blog" %}
        <ul>
          {% for entry in entries %}
            <li>{{ entry.created_at|dateformat(format="short") }} - {{ entry.title }}</li>
          {% endfor %}
        </ul>
      {% endif %}
    {% endfor %}{% endraw %}
  </body>
</html>
```

Would result in this HTML page

```html
<!-- ./dist/blog/index.html -->
<!DOCTYPE html>
<html>
  <head>
    <title>Blog</title>
  </head>

  <body>
    Welcome to my blog.

    <ul>
      <li>2024-06-29 - First post</li>
    </ul>
  </body>
</html>
```
