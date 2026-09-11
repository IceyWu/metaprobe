# Changesets

这个目录使用 [Changesets](https://github.com/changesets/changesets) 管理版本、
CHANGELOG 和 npm 发布。

## 日常开发

每完成一个面向用户的功能、修复或 API 变更，执行：

```bash
pnpm changeset
```

按提示选择版本类型并填写面向用户的说明。Changeset 类型约定：

| 类型 | 使用场景 |
| --- | --- |
| `patch` | Bug 修复、文档、性能优化、内部重构 |
| `minor` | 向后兼容的新功能或新格式支持 |
| `major` | 不兼容的 API、返回结构或行为变更 |

## 发布流程

```bash
pnpm version-packages
pnpm release
```

`version-packages` 会根据待处理的 changeset 更新版本和 `CHANGELOG.md`；
`release` 会重新构建 N-API/WASM 产物、验证发布包，然后执行
`changeset publish`。

GitHub Actions 会在 `main` 分支上自动创建版本 PR。合并版本 PR 后，Action
会执行发布。正式发布前需要在 GitHub 仓库配置 `NPM_TOKEN` secret。

## 首次发布

当前 `package.json` 已准备好以 `0.1.0` 作为首个 npm 版本，首次发布不需要
额外创建一个会把版本号推到 `0.1.1` 的 changeset。后续变更再按日常流程创建
changeset。
