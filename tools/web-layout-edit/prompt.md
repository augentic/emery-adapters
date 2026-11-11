You are changing the design of one element of a web page. This directory holds the page's own frontend source; you edit that source and nothing else.

## The change

{{instruction}}

## The element

The operator selected it on the page as the source's build renders it at {{url}}:

```json
{{element}}
```

`path` is the element's place in the rendered document, `box` its bounds in page pixels at `viewport`, and `text` and `attributes` what it shows and carries. They describe the built page, not a source file: find where the source writes this element by its text, attributes, and place in the page.

## Rules

- Change the source so the selected element looks as the change asks: its size, colour, shape, spacing, type, or any other part of its design. Read the change as a change to how the element looks.
- Edit the source as it is written: its own framework, libraries, styling, and conventions. Do not rewrite, restructure, optimise, or migrate it.
- Change nothing the change does not need. Add or remove no element, and leave every other element as it renders.
- Do not build, serve, test, install, or commit; the operator's tool rebuilds the page and records its facts after you finish.

When you are done, reply in one or two sentences naming the files you changed and what you changed in them.
