# Fracture website

A buildless static landing page for GitHub Pages. Edit `index.html` and
`style.css`; screenshots and the gameplay trailer live in `assets/`.

Preview from the repository root with `python3 -m http.server 8000 --directory site`,
then visit http://localhost:8000. Keep test browsers muted.

The `Publish game website` workflow publishes this directory after changes on
`master`, or when manually dispatched. GitHub Pages must use GitHub Actions as
its build source. No JavaScript, package installation, or build step is required.
