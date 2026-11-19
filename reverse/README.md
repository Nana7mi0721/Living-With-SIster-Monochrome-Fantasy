# 逆向与解包产物

本目录保存《妹相随～黑白世界的缤纷冒险～》的个人解包与逆向资料。仓库为**私有**，仅用于个人研究、汉化与兼容性分析；游戏素材版权归原作者，请勿公开分发。

- 生成日期：2026-11-04
- 源游戏：`E:/SteamLibrary/steamapps/common/Living With SIster Monochrome Fantasy`
- Game.exe 文件版本：`3.191.2023.529`（WOLF RPG Editor）
- 解包工具：UberWolfCli v0.5.4 Build #106

## 目录

| 目录 | 文件数 | 大小 | 内容 |
| --- | ---: | ---: | --- |
| `unpacked/` | 4062 | 735.8 MB | 23 个 `.wolf` 归档的原始解包结果：BGM、SE、Picture、cg、hscene、cutin、BattleEffect、SystemGraphic、DLC_data、BasicData、MapData、SystemFile、scene、word 等 |
| `analysis/` | 466 | 41.6 MB | 调查数据、调试 dump、opcode/格式笔记、参考页与一次性探针脚本归档 |
| `evidence/` | 91 | 37.0 MB | UI/视觉问题证据截图、原版对照截图、评审报告与复现图 |
| `manifest.json` | — | — | 4619 个文件的相对路径、大小与 SHA256 |
| `ARCHIVE_INDEX.md` | — | — | 23 个源归档的大小、解包目录与 SHA256 |

## 重新生成

UberWolfCli 需要 `Game.exe` 与 `Data/` 在同一级目录；把需要解包的 `.wolf` 放在 `Data/` 下，然后执行：

```powershell
UberWolfCli.exe "Data"
```

每个归档会解包到 `Data/<归档名>/`。本目录只提交解包结果，不提交原始 `.wolf` 归档；源归档校验值见 `ARCHIVE_INDEX.md`。

## 说明

- 该分支基于 `master` 的代码树创建，代码与文档仍在原位置；本目录是新增的逆向产物。
- 大文件使用 Git LFS 管理（见仓库根目录 `.gitattributes`）。
- 若只需要游戏资源，可只检出本分支；若需要源码，请使用 `master`。
