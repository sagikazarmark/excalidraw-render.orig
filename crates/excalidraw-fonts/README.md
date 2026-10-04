# excalidraw-fonts

Font payloads and the font catalog used by
[`excalidraw-render`](https://crates.io/crates/excalidraw-render): Excalifont,
Virgil, Nunito, Lilita One, Comic Shanns, Cascadia Code, Liberation Sans, and
Noto Emoji, as shipped by Excalidraw 0.18.1 (the version's build metadata names
the Excalidraw release).

This crate is an implementation detail of `excalidraw-render`, which pins it
exactly. Its API carries no stability guarantee for other consumers.

The default `cjk` feature adds the Xiaolai CJK fallback through
`excalidraw-fonts-xiaolai` and `excalidraw-fonts-xiaolai-extended`.

## License

The crate's code is MIT OR Apache-2.0. The fonts are licensed under the SIL
Open Font License 1.1, except Comic Shanns (MIT). `LICENSE-BUNDLE.txt` contains
every license and notice.
