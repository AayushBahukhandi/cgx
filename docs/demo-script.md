# cgx Demo Video Script

**Target length:** 90 seconds  
**Tool for recording:** Asciinema (terminal) + screen record for web UI  
**Repo to demo on:** `github:expressjs/express` (familiar, mid-size, multi-language)

---

## Pre-recording checklist

- [ ] Terminal font size 18+, high contrast theme
- [ ] Browser zoomed to 125% for web UI shots
- [ ] Run `cgx clean --all` so the analyze step is fresh
- [ ] Pre-auth GitHub token for `cgx share`
- [ ] Close Slack/notifications

---

## Script

### 0:00–0:10 — Hook (text overlay, no typing)

**Text on screen (white on black, 3 seconds each):**
> Reading one source file to answer an architectural question = 15,000–50,000 tokens.
> cgx answers the same question in one command = ~150 tokens.

*Screen: black / repo directory prompt*

---

### 0:10–0:25 — Index a famous repo

```bash
cgx analyze github:expressjs/express
```

Let the parse output fly by. End on the summary line:

```
✓ 847 nodes  2,341 edges  indexed in 4.2s
```

> "cgx indexes your entire repo once — functions, classes, imports, and your full git history."

---

### 0:25–0:40 — The visual (switch to browser recording)

```bash
cgx view --web
```

Pan around the WebGL graph slowly. No voiceover — let it breathe for 5 seconds.

> "This is your codebase as a graph. Every node is a function or module. Every edge is a dependency."

---

### 0:40–0:55 — The two graphs moment

```bash
cgx query blast-radius "Router"
```

Show the real output — whatever the repo produces. Don't fake it.

```bash
cgx hotspots
```

> "That was the structural graph — what calls what.  
> This is the temporal graph — built from git history, not import statements.  
> These two files never import each other, but they change together in 87% of commits.  
> That's hidden coupling. That's where bugs live. No other tool shows you this."

---

### 0:55–1:10 — The AI angle

```bash
cgx setup
```

> "Run cgx setup and your AI editor gets 10 typed tools.  
> get_blast_radius, get_neighbors, get_file_owners — 3 calls, ~200 tokens total.  
> No files opened. Same answer."

---

### 1:10–1:20 — Share + CTA

```bash
cgx share
```

Show the returned URL briefly.

> "Share any repo's graph — no install needed on the other end."

**Text on screen:**
```
brew install aayushbahukhandi/cgx/cgx
github.com/AayushBahukhandi/cgx
```

---

## B-roll ideas (cutaways)

- `cgx complexity --combined` output scrolling
- `cgx deps health --critical` catching a CVE
- `cgx review` generating a PR brief
- `cgx explain --onboard` writing an ARCHITECTURE.md in real time

---

## Post-production notes

- Add captions for the "structural vs temporal" moment — this is the core concept
- Zoom in on the token numbers (~150 vs 15,000–50,000) — make them impossible to miss
- Keep total runtime under 2 minutes; 90 seconds is ideal for Twitter/X
- Export a 60-second cut for Reddit posts
- Export a looping 15-second GIF of `cgx view --web` for the README
