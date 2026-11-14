# Contributing

- Keep it clean-room: never copy or decompile code, textures, sounds or text from Minecraft.
  Describe behaviour from public documentation and implement it from scratch.
- No third-party crates and no bundled binary assets; art is generated in `src/art/`.
- Run `cargo test` before submitting. Add a test for every gameplay rule you implement.
- Check rendering changes with `tools/shot.mjs` and look at the screenshot.
- Document honestly: if something is partial or untested, say so in the README.
