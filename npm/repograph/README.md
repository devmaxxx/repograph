# @devmaxxx/repograph

repograph is now **zerograph**. This package depends on
[`zerograph`](https://www.npmjs.com/package/zerograph), on the public npm registry, and runs it as
`repograph`, so an existing install keeps working. Switch to the new name, which needs no
`.npmrc`:

```sh
pnpm remove @devmaxxx/repograph
pnpm add zerograph
```

The upgrade steps — stores, config and hooks under the old name — are in
[docs/install.md](https://github.com/devmaxxx/zerograph/blob/main/docs/install.md).
