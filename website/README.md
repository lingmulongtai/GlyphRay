# GlyphRay Website

Static GitHub Pages download site for GlyphRay.

## Local Preview

Open `index.html` directly in a browser. The page has no build step and no backend dependency.

## Deployment

The `pages.yml` workflow uploads this directory as a GitHub Pages artifact. GitHub does not always allow the workflow token to create a Pages site, so enable Pages once in repository settings and choose GitHub Actions as the source before rerunning the workflow.
