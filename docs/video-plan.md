# Video Production Plan

## Status: WAITING FOR RECORDINGS

When recordings are done, paste filenames into Claude Code and it will run all ffmpeg commands.

---

## Expected recording files

| File | Content | Duration |
|---|---|---|
| `clip1-terminal.mov` | cgx analyze --force, summary, hotspots, blast-radius "logerror" | ~25 sec |
| `clip2-lazyvim.mov` | AGENTS.md then CGX_SKILL.md open in LazyVim, slow scroll | ~10 sec |
| `clip3-browser.mov` | cgx view --web WebGL graph pan | ~8 sec |
| `clip4-share.mov` | cgx share + URL appearing | ~5 sec |

---

## Text overlays (timestamp → text)

| Timestamp | Overlay text | Style |
|---|---|---|
| 0:00–0:04 | `Reading one source file to understand a codebase` | white, centered |
| 0:04–0:08 | `= 15,000–50,000 tokens` | white, centered |
| 0:08–0:12 | `cgx answers the same question = ~150 tokens` | white, centered |
| 0:25–0:30 | `Structural graph — what calls what` | white, bottom |
| 0:40–0:48 | `146 affected · CRITICAL` | red, large, centered |
| 0:50–0:58 | `Auto-generated on every git commit` | white, bottom |
| 0:58–1:06 | `Your AI reads this instead of opening files` | white, bottom |
| 1:06–1:14 | `Temporal graph — built from git history` | white, bottom |
| 1:18–1:25 | `No install needed to view` | white, bottom |
| 1:25–end | `brew install aayushbahukhandi/cgx/cgx` | white, centered |

---

## ffmpeg plan (Claude runs this once recordings arrive)

1. Stitch all 4 clips into one `.mov`
2. Burn text overlays via `drawtext` filter
3. Export README GIF at 1200px wide, 12fps
4. Export Twitter/X cut (60 sec) as separate GIF
5. Place final GIF at `docs/demo.gif`
