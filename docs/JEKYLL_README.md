# Cardano Rust Node - Documentation Site

This directory contains the Jekyll-based documentation site for Cardano Rust Node, automatically deployed to GitHub Pages.

## 🌐 Live Site

The documentation is automatically deployed to: **https://fractionestate.github.io/cardano-rust-node**

## 📚 Documentation Structure

```
docs/
├── _config.yml           # Jekyll configuration
├── Gemfile              # Ruby dependencies
├── index.md             # Home page
├── QUICKSTART.md        # Quick start guide
├── api/                 # API & CLI documentation
│   ├── index.md
│   ├── API_REFERENCE.md
│   └── CLI_REFERENCE.md
├── architecture/        # Architecture documentation
│   ├── index.md
│   └── *.md
├── guides/              # User guides
│   ├── index.md
│   └── *.md
├── operations/          # Operations documentation
├── reference/           # Quick references
├── development/         # Development guides
└── reports/             # Status reports and audits
    ├── audits/
    ├── protocol/
    ├── testing/
    └── archive/
```

## 🎨 Theme

We use the **[Just the Docs](https://just-the-docs.github.io/just-the-docs/)** Jekyll theme, which provides:

- 🔍 **Built-in search** - Fast documentation search
- 📱 **Responsive design** - Mobile-friendly
- 🎨 **Clean interface** - Professional documentation layout
- 🌳 **Nested navigation** - Hierarchical organization
- 🔗 **Anchor links** - Deep linking to sections
- 📝 **Code highlighting** - Syntax highlighting for code blocks
- ⚡ **Fast** - Optimized for performance

## 🚀 Local Development

### Prerequisites

- Ruby 3.0+
- Bundler

### Setup

```bash
# Navigate to docs directory
cd docs

# Install dependencies
bundle install

# Serve locally
bundle exec jekyll serve

# View at http://localhost:4000
```

### Live Reload

The local server supports live reload - changes to markdown files will automatically refresh the browser.

## 📝 Adding New Pages

### Basic Page

Create a new `.md` file with front matter:

```markdown
---
layout: default
title: My Page Title
nav_order: 10
description: "Page description for SEO"
permalink: /my-page/
---

# My Page Title

Content goes here...
```

### Page with Children

For section index pages:

```markdown
---
layout: default
title: Section Name
nav_order: 5
has_children: true
permalink: /section/
---

# Section Name

Section overview...
```

### Child Page

```markdown
---
layout: default
title: Child Page
parent: Section Name
nav_order: 1
---

# Child Page

Content...
```

## 🎨 Customization

### Colors

Edit `_config.yml` to change the color scheme:

```yaml
color_scheme: light  # Options: light, dark, or custom
```

### Logo

Add your logo to `/assets/images/` and update `_config.yml`:

```yaml
logo: "/assets/images/cardano-logo.png"
```

### Navigation

Navigation is automatically generated from page front matter. Control order with `nav_order`:

```yaml
nav_order: 1  # Lower numbers appear first
```

## 📊 Special Features

### Callouts

Use callouts for important information:

```markdown
{: .note }
This is a note callout.

{: .warning }
This is a warning callout.

{: .important }
This is an important callout.

{: .tip }
This is a tip callout.
```

### Code Blocks

With syntax highlighting:

````markdown
```rust
fn main() {
    println!("Hello, Cardano!");
}
```
````

### Tables

```markdown
| Header 1 | Header 2 |
|:---------|:---------|
| Cell 1   | Cell 2   |
```

### Task Lists

```markdown
- [x] Completed task
- [ ] Pending task
```

### Buttons

```markdown
[Button Text](link){: .btn .btn-primary }
```

## 🔗 Internal Links

Link to other pages using Jekyll's link tag:

```markdown
[Getting Started]({% link guides/GETTING_STARTED.md %})
```

## 🚢 Deployment

### Automatic Deployment

- Documentation is automatically deployed via GitHub Actions
- Triggered on push to `001-cardano-node-rust-rewrite` branch
- Workflow file: `.github/workflows/docs.yml`

### Manual Deployment

GitHub Pages can be configured in repository settings:
1. Go to Settings > Pages
2. Set Source to "GitHub Actions"
3. The workflow will handle the rest

## 📋 Front Matter Reference

Common front matter options:

```yaml
---
layout: default              # Page layout (usually 'default')
title: Page Title           # Page title (required)
nav_order: 1                # Navigation order (optional)
description: "Description"  # SEO description (optional)
permalink: /custom-url/     # Custom URL (optional)
parent: Parent Page         # Parent page for nested nav (optional)
has_children: true          # Has child pages (optional)
grand_parent: Grand Parent  # Grandparent page (optional)
nav_exclude: true          # Exclude from navigation (optional)
search_exclude: true       # Exclude from search (optional)
---
```

## 🔍 Search

Search is automatically configured and will index all content. To exclude a page from search:

```yaml
search_exclude: true
```

## 📦 Dependencies

Key dependencies (see `Gemfile`):

- `github-pages` - GitHub Pages gem with Jekyll
- `jekyll-feed` - RSS feed generation
- `jekyll-seo-tag` - SEO optimization
- `jekyll-sitemap` - Sitemap generation
- `just-the-docs` - Documentation theme

## 🛠️ Troubleshooting

### Build Errors

```bash
# Clear Jekyll cache
bundle exec jekyll clean

# Rebuild
bundle exec jekyll build
```

### Dependency Issues

```bash
# Update dependencies
bundle update

# Reinstall
rm Gemfile.lock
bundle install
```

### Local Preview Issues

```bash
# Serve with verbose output
bundle exec jekyll serve --verbose

# Serve with livereload
bundle exec jekyll serve --livereload
```

## 📚 Resources

- [Jekyll Documentation](https://jekyllrb.com/docs/)
- [Just the Docs Theme](https://just-the-docs.github.io/just-the-docs/)
- [GitHub Pages Documentation](https://docs.github.com/en/pages)
- [Markdown Guide](https://www.markdownguide.org/)

## 🤝 Contributing

When adding documentation:

1. Follow the existing structure
2. Add appropriate front matter
3. Use clear, concise language
4. Include code examples where relevant
5. Test locally before committing
6. Update navigation as needed

## 📄 License

Documentation is licensed under the same license as the main project (MIT).

---

For questions or issues with the documentation site, please open an issue on GitHub.
