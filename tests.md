# TESTS.md — Phase Verification Scenarios

> Run these after EVERY phase before moving to the next one.
> If any scenario fails, fix it before continuing.
> Claude Code must not proceed to Phase N+1 if Phase N has failing scenarios.

---

## How to Use This File

After Claude Code finishes a phase, tell it:
```
"Run the Phase N verification scenarios from TESTS.md and show me the output."
```

If output matches the expected output, move on.
If it doesn't, paste the actual output back and say:
```
"This scenario failed. Expected X, got Y. Fix it without moving to the next phase."
```

---

## Fixture Repos (Used Across All Phases)

Before any testing, Claude Code must create these fixture repos.
Tell it: *"Create all fixture repos described in TESTS.md before running any scenarios."*

### Fixture A — `tests/fixtures/ts-sample/`

```
ts-sample/
├── src/
│   ├── auth.ts
│   ├── router.ts
│   ├── user.ts
│   └── db.ts
└── package.json
```

**`src/auth.ts`**
```typescript
import { hashPassword } from './user';
import { query } from './db';

export class AuthService {
  async login(email: string, password: string): Promise<boolean> {
    const hashed = hashPassword(password);
    const result = await query('SELECT * FROM users WHERE email = ?', [email]);
    return result.length > 0;
  }

  async logout(sessionId: string): Promise<void> {
    await query('DELETE FROM sessions WHERE id = ?', [sessionId]);
  }
}

export function validateToken(token: string): boolean {
  return token.length === 64;
}
```

**`src/user.ts`**
```typescript
import { query } from './db';

export class UserService {
  async getUser(id: string) {
    return await query('SELECT * FROM users WHERE id = ?', [id]);
  }

  async deleteUser(id: string) {
    await query('DELETE FROM users WHERE id = ?', [id]);
  }
}

export function hashPassword(password: string): string {
  return Buffer.from(password).toString('base64');
}
```

**`src/db.ts`**
```typescript
export async function query(sql: string, params: any[] = []): Promise<any[]> {
  return [];
}

export function getConnection() {
  return { connected: true };
}
```

**`src/router.ts`**
```typescript
import { AuthService, validateToken } from './auth';
import { UserService } from './user';

const auth = new AuthService();
const users = new UserService();

export async function handleLogin(email: string, password: string) {
  return await auth.login(email, password);
}

export async function handleGetUser(id: string, token: string) {
  if (!validateToken(token)) throw new Error('Invalid token');
  return await users.getUser(id);
}

export async function handleDeleteUser(id: string) {
  return await users.deleteUser(id);
}
```

---

### Fixture B — `tests/fixtures/py-sample/`

```
py-sample/
├── auth.py
├── models.py
└── api.py
```

**`auth.py`**
```python
from models import User

class AuthService:
    def login(self, email: str, password: str) -> bool:
        user = User.find_by_email(email)
        return user is not None

    def logout(self, session_id: str) -> None:
        pass

def hash_password(password: str) -> str:
    return password[::-1]
```

**`models.py`**
```python
class User:
    def __init__(self, email: str, name: str):
        self.email = email
        self.name = name

    @classmethod
    def find_by_email(cls, email: str):
        return None

class Session:
    def __init__(self, user_id: str):
        self.user_id = user_id

    def is_valid(self) -> bool:
        return True
```

**`api.py`**
```python
from auth import AuthService, hash_password
from models import User, Session

auth = AuthService()

def login_endpoint(email: str, password: str):
    return auth.login(email, password)

def get_user_endpoint(user_id: str):
    return User.find_by_email(user_id)

def create_session(user_id: str):
    return Session(user_id)
```

---

### Fixture C — `tests/fixtures/rust-sample/`

```
rust-sample/
├── Cargo.toml
└── src/
    ├── main.rs
    ├── auth.rs
    └── db.rs
```

**`Cargo.toml`**
```toml
[package]
name = "rust-sample"
version = "0.1.0"
edition = "2021"
```

**`src/auth.rs`**
```rust
use crate::db;

pub struct AuthService {
    pub session_timeout: u64,
}

impl AuthService {
    pub fn new() -> Self {
        Self { session_timeout: 3600 }
    }

    pub fn login(&self, email: &str, password: &str) -> bool {
        db::find_user(email).is_some()
    }

    pub fn logout(&self, session_id: &str) -> bool {
        db::delete_session(session_id)
    }
}

pub fn validate_token(token: &str) -> bool {
    token.len() == 64
}
```

**`src/db.rs`**
```rust
pub struct User {
    pub email: String,
    pub name: String,
}

pub fn find_user(email: &str) -> Option<User> {
    None
}

pub fn delete_session(session_id: &str) -> bool {
    true
}
```

**`src/main.rs`**
```rust
mod auth;
mod db;

fn main() {
    let service = auth::AuthService::new();
    let result = service.login("test@test.com", "password");
    println!("Login result: {}", result);
}
```

---

### Fixture D — `tests/fixtures/git-sample/` (Git repo with history)

Claude Code must initialize this as an actual git repo with commits:

```bash
mkdir -p tests/fixtures/git-sample/src
cd tests/fixtures/git-sample
git init
git config user.email "alice@dev.io"
git config user.name "Alice"

# Commit 1 — Alice adds auth
cat > src/auth.ts << 'EOF'
export function login(email: string) { return true; }
EOF
cat > src/db.ts << 'EOF'
export function query(sql: string) { return []; }
EOF
git add . && git commit -m "feat: add auth and db"

# Commit 2 — Alice modifies both auth and db together
cat > src/auth.ts << 'EOF'
export function login(email: string) { return true; }
export function logout(id: string) { return true; }
EOF
cat > src/db.ts << 'EOF'
export function query(sql: string) { return []; }
export function connect() { return true; }
EOF
git add . && git commit -m "feat: add logout and connect"

# Commit 3 — Bob adds router
git config user.email "bob@dev.io"
git config user.name "Bob"
cat > src/router.ts << 'EOF'
import { login } from './auth';
export function handleLogin() { return login('test'); }
EOF
git add . && git commit -m "feat: add router"

# Commit 4 — Alice modifies auth and db together again (co-change)
git config user.email "alice@dev.io"
git config user.name "Alice"
cat >> src/auth.ts << 'EOF'
export function resetPassword(email: string) { return true; }
EOF
cat >> src/db.ts << 'EOF'
export function transaction(fn: Function) { return fn(); }
EOF
git add . && git commit -m "feat: add resetPassword and transaction"
```

After setup, `git-sample` should have:
- 4 commits
- 2 authors (Alice, Bob)
- `auth.ts` and `db.ts` changed together in commits 1, 2, and 4 (co-change count = 3)
- `router.ts` only changed in commit 3

---

## Phase 0 — Scaffold Verification

### Scenario 0.1 — Workspace compiles
```bash
cargo build --workspace 2>&1
```
**Expected:** Last line contains `Finished` with zero errors. No `error[E...]` lines.

### Scenario 0.2 — Web UI builds
```bash
npm run build 2>&1 | tail -5
```
**Expected:** Output contains `dist/` and `built in` with no ERROR lines.

### Scenario 0.3 — Directory structure matches spec
```bash
find . -type f -name "*.rs" | sort
find . -type f -name "*.ts" | grep -v node_modules | grep -v dist | sort
```
**Expected RS files (minimum):**
```
./crates/cgx-cli/src/main.rs
./crates/cgx-core/src/lib.rs
./crates/cgx-mcp/src/main.rs
```
**Expected TS files (minimum):**
```
./packages/web-ui/src/main.tsx
./packages/web-ui/src/App.tsx
```

### Scenario 0.4 — Binary runs
```bash
cargo run -p cgx-cli -- --help 2>&1
```
**Expected:** Help text printed. Exit code 0. No panics.

---

## Phase 1 — Parser Verification

### Scenario 1.1 — TypeScript fixture extracts correct symbols
```bash
cargo run -p cgx-cli -- parse tests/fixtures/ts-sample --json 2>/dev/null
```
**Expected JSON must contain all of these node names:**
```
AuthService
UserService
login          (×2 — one in auth.ts, one in user.ts... wait, user has getUser)
logout
validateToken
hashPassword
handleLogin
handleGetUser
handleDeleteUser
getUser
deleteUser
query
getConnection
```

Run this to verify:
```bash
cargo run -p cgx-cli -- parse tests/fixtures/ts-sample --json 2>/dev/null \
  | python3 -c "
import sys, json
data = json.load(sys.stdin)
names = {n['name'] for n in data['nodes']}
required = {'AuthService','UserService','validateToken','hashPassword',
            'handleLogin','handleGetUser','handleDeleteUser','query','getConnection'}
missing = required - names
if missing:
    print('FAIL — missing nodes:', missing)
else:
    print('PASS — all required nodes found')
    print(f'Total nodes: {len(data[\"nodes\"])}')
"
```
**Expected:** `PASS — all required nodes found`, `Total nodes:` >= 12

### Scenario 1.2 — Python fixture extracts correct symbols
```bash
cargo run -p cgx-cli -- parse tests/fixtures/py-sample --json 2>/dev/null \
  | python3 -c "
import sys, json
data = json.load(sys.stdin)
names = {n['name'] for n in data['nodes']}
required = {'AuthService','User','Session','login','logout',
            'hash_password','find_by_email','is_valid',
            'login_endpoint','get_user_endpoint','create_session'}
missing = required - names
if missing:
    print('FAIL — missing:', missing)
else:
    print('PASS')
print('Total:', len(data['nodes']))
"
```
**Expected:** `PASS`, total >= 10

### Scenario 1.3 — Rust fixture extracts correct symbols
```bash
cargo run -p cgx-cli -- parse tests/fixtures/rust-sample --json 2>/dev/null \
  | python3 -c "
import sys, json
data = json.load(sys.stdin)
names = {n['name'] for n in data['nodes']}
required = {'AuthService','login','logout','validate_token',
            'User','find_user','delete_session'}
missing = required - names
if missing:
    print('FAIL — missing:', missing)
else:
    print('PASS')
"
```
**Expected:** `PASS`

### Scenario 1.4 — No panic on unknown file types
```bash
echo "this is not code" > /tmp/test.xyz
cargo run -p cgx-cli -- parse /tmp/ --json 2>/dev/null
echo "Exit code: $?"
```
**Expected:** Exit code 0. No panic. Empty or minimal JSON output.

### Scenario 1.5 — Parallel parsing is faster than sequential on large input
```bash
# Clone a real mid-size TS repo for this test
git clone --depth=1 https://github.com/expressjs/express /tmp/express-test 2>/dev/null
time cargo run -p cgx-cli -- parse /tmp/express-test 2>/dev/null
```
**Expected:** Completes in under 10 seconds. Prints node count > 50.

### Scenario 1.6 — Import edges are present in output
```bash
cargo run -p cgx-cli -- parse tests/fixtures/ts-sample --json 2>/dev/null \
  | python3 -c "
import sys, json
data = json.load(sys.stdin)
import_edges = [e for e in data['edges'] if e['kind'] == 'IMPORTS']
if len(import_edges) < 3:
    print('FAIL — expected at least 3 import edges, got', len(import_edges))
else:
    print('PASS —', len(import_edges), 'import edges found')
    for e in import_edges[:3]:
        print(' ', e['src'], '->', e['dst'])
"
```
**Expected:** `PASS — 3+ import edges found`

### Cargo Unit Tests — Phase 1
```bash
cargo test -p cgx-core 2>&1 | tail -20
```
**Expected:** `test result: ok. N passed; 0 failed`

---

## Phase 2 — Storage + CLI Verification

### Scenario 2.1 — `cgx analyze` runs and creates DB file
```bash
cargo run -p cgx-cli -- analyze tests/fixtures/ts-sample 2>&1
ls -la ~/.cgx/repos/ 2>&1
```
**Expected:**
- Output contains `✓ Done` or similar success message
- `~/.cgx/repos/` contains at least one `.db` file
- No ERROR or panic in output

### Scenario 2.2 — Node and edge counts are correct
```bash
cargo run -p cgx-cli -- status tests/fixtures/ts-sample 2>&1
```
**Expected output contains:**
```
Nodes: [number >= 12]
Edges: [number >= 8]
```

### Scenario 2.3 — Registry is updated
```bash
cat ~/.cgx/registry.json | python3 -c "
import sys, json
data = json.load(sys.stdin)
if not data.get('repos'):
    print('FAIL — no repos in registry')
else:
    r = data['repos'][0]
    print('PASS')
    print('Registered repo:', r['name'])
    print('Node count:', r['node_count'])
"
```
**Expected:** `PASS`, shows repo name and node count

### Scenario 2.4 — `cgx list` shows indexed repos
```bash
cargo run -p cgx-cli -- list 2>&1
```
**Expected:** Table with at least 1 row. Columns: name, path, nodes, indexed_at.
Must not be empty or crash.

### Scenario 2.5 — Re-running analyze doesn't crash (idempotent)
```bash
cargo run -p cgx-cli -- analyze tests/fixtures/ts-sample 2>&1
cargo run -p cgx-cli -- analyze tests/fixtures/ts-sample 2>&1
echo "Exit: $?"
```
**Expected:** Both runs succeed. Exit code 0. Second run says "already indexed" or re-indexes cleanly.

### Scenario 2.6 — Cross-file IMPORTS edges are in DB
```bash
# After analyze, query the DB directly
cargo run -p cgx-cli -- query "show imports for src/router.ts" 2>&1
```
**Expected output contains:**
- `auth.ts` or `auth` (router imports from auth)
- `user.ts` or `user` (router imports from user)

Alternative if query command not yet built:
```bash
cargo run -p cgx-cli -- export --format=json 2>/dev/null \
  | python3 -c "
import sys, json
data = json.load(sys.stdin)
import_edges = [e for e in data['edges'] if e['kind'] == 'IMPORTS']
router_imports = [e for e in import_edges if 'router' in e['src']]
if len(router_imports) < 2:
    print('FAIL — router should import from at least 2 files, got', len(router_imports))
else:
    print('PASS —', len(router_imports), 'imports from router')
"
```

### Scenario 2.7 — Non-git folder doesn't crash
```bash
mkdir -p /tmp/not-a-git-repo
echo "export const x = 1;" > /tmp/not-a-git-repo/index.ts
cargo run -p cgx-cli -- analyze /tmp/not-a-git-repo 2>&1
echo "Exit: $?"
```
**Expected:** Completes successfully. May warn "not a git repository". Exit code 0.

### Cargo Unit Tests — Phase 2
```bash
cargo test -p cgx-core test_graph 2>&1
cargo test -p cgx-core test_resolver 2>&1
```
**Expected:** All tests pass, 0 failed.

---

## Phase 3 — Git Intelligence Verification

### Scenario 3.1 — Churn scores are populated after analyze
```bash
cargo run -p cgx-cli -- analyze tests/fixtures/git-sample 2>&1
cargo run -p cgx-cli -- export --format=json --repo=git-sample 2>/dev/null \
  | python3 -c "
import sys, json
data = json.load(sys.stdin)
file_nodes = [n for n in data['nodes'] if n['kind'] == 'File']
with_churn = [n for n in file_nodes if n.get('churn', 0) > 0]
print(f'File nodes: {len(file_nodes)}')
print(f'With churn > 0: {len(with_churn)}')
if len(with_churn) == 0:
    print('FAIL — no churn scores populated')
else:
    print('PASS')
    for n in sorted(with_churn, key=lambda x: -x['churn'])[:3]:
        print(f'  {n[\"name\"]}: churn={n[\"churn\"]:.2f}')
"
```
**Expected:** `PASS`, auth.ts and db.ts should have highest churn (changed most)

### Scenario 3.2 — CO_CHANGES edges exist between auth.ts and db.ts
```bash
cargo run -p cgx-cli -- export --format=json --repo=git-sample 2>/dev/null \
  | python3 -c "
import sys, json
data = json.load(sys.stdin)
co_edges = [e for e in data['edges'] if e['kind'] == 'CO_CHANGES']
print(f'CO_CHANGES edges: {len(co_edges)}')
auth_db = [e for e in co_edges
           if ('auth' in e['src'] and 'db' in e['dst'])
           or ('db' in e['src'] and 'auth' in e['dst'])]
if not auth_db:
    print('FAIL — no CO_CHANGES edge between auth.ts and db.ts')
    print('All co-change edges:')
    for e in co_edges: print(' ', e['src'], '<->', e['dst'])
else:
    print('PASS — auth.ts <-> db.ts co-change edge found')
    print('Weight:', auth_db[0]['weight'])
"
```
**Expected:** `PASS — auth.ts <-> db.ts co-change edge found`, weight > 0.5

### Scenario 3.3 — OWNS edges and Author nodes exist
```bash
cargo run -p cgx-cli -- export --format=json --repo=git-sample 2>/dev/null \
  | python3 -c "
import sys, json
data = json.load(sys.stdin)
authors = [n for n in data['nodes'] if n['kind'] == 'Author']
owns = [e for e in data['edges'] if e['kind'] == 'OWNS']
print(f'Author nodes: {len(authors)}')
print(f'OWNS edges: {len(owns)}')
if len(authors) < 2:
    print('FAIL — expected at least 2 authors (Alice + Bob)')
elif len(owns) == 0:
    print('FAIL — no OWNS edges')
else:
    print('PASS')
    for a in authors:
        print(f'  Author: {a[\"name\"]}')
"
```
**Expected:** `PASS`, 2 authors: Alice and Bob

### Scenario 3.4 — `cgx hotspots` prints a ranked list
```bash
# First analyze a real-world repo with enough history
git clone --depth=50 https://github.com/expressjs/express /tmp/express-hotspot 2>/dev/null
cargo run -p cgx-cli -- analyze /tmp/express-hotspot 2>&1 | tail -5
cargo run -p cgx-cli -- hotspots --repo=/tmp/express-hotspot 2>&1
```
**Expected:** A table with at least 3 rows. Columns: rank, file, churn, coupling, callers.
Numbers must be non-zero.

### Scenario 3.5 — `cgx blame-graph` shows ownership
```bash
cargo run -p cgx-cli -- blame-graph --repo=tests/fixtures/git-sample 2>&1
```
**Expected output contains:**
```
alice@dev.io   [some bar]  [some %]
bob@dev.io     [some bar]  [some %]
```
Alice should own more than Bob (she made 3 of 4 commits).

### Cargo Unit Tests — Phase 3
```bash
cargo test -p cgx-core test_git 2>&1
```
**Expected:** All pass. Fixture git-sample is used by these tests.

---

## Phase 4 — Clustering Verification

### Scenario 4.1 — Every node has a community after analyze
```bash
cargo run -p cgx-cli -- export --format=json 2>/dev/null \
  | python3 -c "
import sys, json
data = json.load(sys.stdin)
nodes = data['nodes']
without = [n for n in nodes if n.get('community') is None]
print(f'Total nodes: {len(nodes)}')
print(f'Without community: {len(without)}')
if without:
    print('FAIL — nodes missing community:', [n[\"name\"] for n in without[:5]])
else:
    print('PASS — all nodes have community assigned')
communities = set(n['community'] for n in nodes)
print(f'Unique communities: {len(communities)}')
"
```
**Expected:** `PASS`, at least 2 unique communities

### Scenario 4.2 — Related nodes cluster together
```bash
cargo run -p cgx-cli -- export --format=json 2>/dev/null \
  | python3 -c "
import sys, json
data = json.load(sys.stdin)
# auth.ts and authService should be in the same community
nodes_by_name = {n['name']: n for n in data['nodes']}
auth_class = nodes_by_name.get('AuthService')
validate_fn = nodes_by_name.get('validateToken')
if not auth_class or not validate_fn:
    print('SKIP — fixture nodes not found')
elif auth_class['community'] == validate_fn['community']:
    print('PASS — AuthService and validateToken are in same community:', auth_class['community'])
else:
    print('WARN — AuthService community:', auth_class['community'],
          'validateToken community:', validate_fn['community'])
    print('This may be acceptable depending on graph density')
"
```
**Expected:** `PASS` or `WARN` (WARN is acceptable — clustering is probabilistic)

### Scenario 4.3 — Clustering is stable across re-runs
```bash
cargo run -p cgx-cli -- analyze tests/fixtures/ts-sample --force 2>/dev/null
cargo run -p cgx-cli -- export --format=json 2>/dev/null \
  | python3 -c "import sys,json; d=json.load(sys.stdin); print({n['name']:n['community'] for n in d['nodes'] if n['kind']=='Class'})" > /tmp/run1.txt

cargo run -p cgx-cli -- analyze tests/fixtures/ts-sample --force 2>/dev/null
cargo run -p cgx-cli -- export --format=json 2>/dev/null \
  | python3 -c "import sys,json; d=json.load(sys.stdin); print({n['name']:n['community'] for n in d['nodes'] if n['kind']=='Class'})" > /tmp/run2.txt

diff /tmp/run1.txt /tmp/run2.txt && echo "PASS — stable" || echo "FAIL — unstable clustering"
```
**Expected:** `PASS — stable`

---

## Phase 5 — Export Format Verification

### Scenario 5.1 — JSON export is valid and complete
```bash
cargo run -p cgx-cli -- export --format=json --out=/tmp/test-graph.json 2>&1
python3 -c "
import json
with open('/tmp/test-graph.json') as f:
    data = json.load(f)

required_keys = ['meta', 'nodes', 'edges', 'communities']
missing = [k for k in required_keys if k not in data]
if missing:
    print('FAIL — missing keys:', missing)
else:
    print('PASS — valid JSON structure')
    print('Nodes:', len(data['nodes']))
    print('Edges:', len(data['edges']))
    print('Communities:', len(data['communities']))

# Spot check node schema
n = data['nodes'][0]
node_fields = ['id', 'kind', 'name', 'path', 'churn', 'coupling', 'community']
missing_fields = [f for f in node_fields if f not in n]
if missing_fields:
    print('FAIL — node missing fields:', missing_fields)
else:
    print('PASS — node schema correct')
"
```
**Expected:** Both `PASS` lines

### Scenario 5.2 — Mermaid export is parseable
```bash
cargo run -p cgx-cli -- export --format=mermaid 2>/dev/null > /tmp/test.mmd
head -5 /tmp/test.mmd
grep -c "\-\->" /tmp/test.mmd
```
**Expected:**
- First line is `graph TD` or `graph LR`
- Arrow count (`-->`) is > 0
- File is not empty

### Scenario 5.3 — DOT export is valid Graphviz syntax
```bash
cargo run -p cgx-cli -- export --format=dot --out=/tmp/test.dot 2>&1
head -3 /tmp/test.dot
# If graphviz is installed:
which dot && dot -Tsvg /tmp/test.dot > /dev/null && echo "PASS — valid DOT" || echo "dot not installed, check manually"
```
**Expected:** First line is `digraph {` or `digraph G {`

### Scenario 5.4 — GraphML export has correct structure
```bash
cargo run -p cgx-cli -- export --format=graphml --out=/tmp/test.graphml 2>&1
python3 -c "
import xml.etree.ElementTree as ET
try:
    tree = ET.parse('/tmp/test.graphml')
    root = tree.getroot()
    ns = '{http://graphml.graphdrawing.org/graphml}'
    nodes = root.findall(f'.//{ns}node')
    edges = root.findall(f'.//{ns}edge')
    print(f'PASS — {len(nodes)} nodes, {len(edges)} edges in GraphML')
except Exception as e:
    print('FAIL —', e)
"
```
**Expected:** `PASS — N nodes, M edges in GraphML`

### Scenario 5.5 — JSON round-trip integrity
```bash
cargo run -p cgx-cli -- export --format=json --out=/tmp/graph1.json 2>/dev/null
python3 -c "
import json
with open('/tmp/graph1.json') as f:
    d = json.load(f)
node_ids = {n['id'] for n in d['nodes']}
edge_srcs = {e['src'] for e in d['edges']}
# All edge sources must exist as nodes
dangling = edge_srcs - node_ids
if dangling:
    print('FAIL — dangling edge sources:', list(dangling)[:5])
else:
    print('PASS — all edges reference valid nodes')
"
```
**Expected:** `PASS — all edges reference valid nodes`

---

## Phase 6 — TUI Verification

> TUI is visual — automated testing is limited. Use these manual checks.

### Scenario 6.1 — TUI launches without crashing
```bash
timeout 3 cargo run -p cgx-cli -- view 2>&1
echo "Exit: $?"
```
**Expected:** Output shows TUI rendering attempt. May exit with timeout (code 124) which is fine.
Must NOT exit with panic (code 101) or segfault (code 139).

### Scenario 6.2 — TUI with community filter scopes correctly
```bash
timeout 3 cargo run -p cgx-cli -- view --community=0 2>&1
echo "Exit: $?"
```
**Expected:** Exits via timeout (not panic). No error about invalid community ID.

### Scenario 6.3 — TUI handles empty graph gracefully
```bash
mkdir -p /tmp/empty-repo
timeout 3 cargo run -p cgx-cli -- view --repo=/tmp/empty-repo 2>&1
echo "Exit: $?"
```
**Expected:** Shows "no graph indexed" message or empty state. Does NOT panic.

### Manual Checklist (run `cgx view` and verify by eye):
```
[ ] Graph renders with visible nodes (dots or Unicode blocks)
[ ] Node colors differ by kind (functions vs classes vs files)
[ ] Arrow keys or hjkl move selection between nodes
[ ] Pressing Enter or Tab selects a node
[ ] Right panel updates with selected node's name, kind, file
[ ] Churn bar shows a non-empty bar for high-churn nodes
[ ] Pressing / enters search mode
[ ] Pressing q quits cleanly (no zombie process)
[ ] Resizing terminal doesn't crash
[ ] --filter=src scopes visible nodes to that path
```

---

## Phase 7 — Web UI Verification

### Scenario 7.1 — `cgx serve` starts HTTP server
```bash
timeout 5 cargo run -p cgx-cli -- serve 2>&1 &
sleep 2
curl -s http://localhost:7373/api/graph | python3 -c "
import sys, json
try:
    data = json.load(sys.stdin)
    print('PASS — API returned valid JSON')
    print('Nodes:', len(data.get('nodes', [])))
except:
    print('FAIL — invalid JSON or server not running')
"
kill %1 2>/dev/null
```
**Expected:** `PASS — API returned valid JSON`, node count > 0

### Scenario 7.2 — API endpoint for repos list works
```bash
timeout 5 cargo run -p cgx-cli -- serve 2>&1 &
sleep 2
curl -s http://localhost:7373/api/repos | python3 -c "
import sys, json
data = json.load(sys.stdin)
if isinstance(data, list) and len(data) > 0:
    print('PASS —', len(data), 'repos listed')
else:
    print('FAIL — expected list of repos, got:', data)
"
kill %1 2>/dev/null
```
**Expected:** `PASS — N repos listed`

### Scenario 7.3 — Web UI build includes graph data injection
```bash
# Build web UI with graph injected
cargo run -p cgx-cli -- export --format=json --out=/tmp/inject-test.json 2>/dev/null
npm run build --workspace=packages/web-ui 2>/dev/null
grep -l "__CGX_GRAPH__" packages/web-ui/dist/*.html 2>/dev/null \
  && echo "PASS — graph data injected into HTML" \
  || echo "INFO — graph injected at serve time (also acceptable)"
```

### Manual Checklist (run `cgx view --web` and verify by eye):
```
[ ] Browser opens automatically to http://localhost:7373
[ ] Graph canvas renders with nodes visible
[ ] Nodes have different colors by type (green=function, blue=class, etc.)
[ ] Node sizes visually differ (larger = higher churn)
[ ] Clicking a node opens the inspector panel on the right
[ ] Inspector panel shows node name, kind, file path
[ ] Search bar filters visible nodes in real-time
[ ] Community dropdown scopes the graph
[ ] Edge type toggles show/hide CALLS, IMPORTS, CO_CHANGES
[ ] Force layout runs and graph stabilizes (not all nodes in center)
[ ] Page works with graph files up to 10MB without freezing
[ ] Dark background (#0a0a0f), JetBrains Mono font visible
[ ] No CORS errors in browser console
```

---

## Phase 8 — MCP Server Verification

### Scenario 8.1 — MCP server responds to initialize
```bash
echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"1.0"}}}' \
  | timeout 3 cargo run -p cgx-mcp -- 2>/dev/null \
  | python3 -c "
import sys, json
line = sys.stdin.readline()
data = json.loads(line)
if data.get('result'):
    print('PASS — initialize responded')
    print('Server name:', data['result'].get('serverInfo', {}).get('name'))
else:
    print('FAIL — unexpected response:', data)
"
```
**Expected:** `PASS — initialize responded`

### Scenario 8.2 — tools/list returns all 10 tools
```bash
(
echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"1.0"}}}'
echo '{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}'
) | timeout 3 cargo run -p cgx-mcp -- 2>/dev/null \
  | python3 -c "
import sys, json
lines = sys.stdin.readlines()
for line in lines:
    try:
        data = json.loads(line)
        if data.get('id') == 2:
            tools = data['result']['tools']
            names = {t['name'] for t in tools}
            required = {'get_repo_summary','find_symbol','get_neighbors','get_call_chain',
                        'get_blast_radius','get_community','search_graph',
                        'get_hotspots','get_file_owners','run_query'}
            missing = required - names
            if missing:
                print('FAIL — missing tools:', missing)
            else:
                print('PASS — all 10 tools registered')
    except: pass
"
```
**Expected:** `PASS — all 10 tools registered`

### Scenario 8.3 — find_symbol tool works
```bash
(
echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"1.0"}}}'
echo '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"find_symbol","arguments":{"name":"AuthService"}}}'
) | timeout 5 cargo run -p cgx-mcp -- 2>/dev/null \
  | python3 -c "
import sys, json
for line in sys.stdin:
    try:
        data = json.loads(line)
        if data.get('id') == 2:
            content = data['result']['content']
            text = content[0]['text'] if content else ''
            parsed = json.loads(text)
            if parsed.get('nodes') and len(parsed['nodes']) > 0:
                print('PASS — find_symbol returned', len(parsed['nodes']), 'nodes')
            else:
                print('FAIL — no nodes returned for AuthService')
    except Exception as e: pass
"
```
**Expected:** `PASS — find_symbol returned N nodes`

### Scenario 8.4 — run_query blocks dangerous SQL
```bash
(
echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"1.0"}}}'
echo '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"run_query","arguments":{"sql":"DELETE FROM nodes"}}}'
) | timeout 3 cargo run -p cgx-mcp -- 2>/dev/null \
  | python3 -c "
import sys, json
for line in sys.stdin:
    try:
        data = json.loads(line)
        if data.get('id') == 2:
            if data.get('error') or 'not allowed' in str(data).lower() or 'read-only' in str(data).lower():
                print('PASS — dangerous SQL blocked')
            else:
                print('FAIL — DELETE was not blocked!')
    except: pass
"
```
**Expected:** `PASS — dangerous SQL blocked`

### Scenario 8.5 — `cgx setup` detects editors
```bash
cargo run -p cgx-cli -- setup --dry-run 2>&1
```
**Expected:** Lists which editors were detected and which config files would be written.
Must not crash if no editors are installed.

### Scenario 8.6 — `get_repo_summary` tool returns full overview
```bash
(
echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"1.0"}}}'
echo '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"get_repo_summary","arguments":{}}}'
) | timeout 5 cargo run -p cgx-mcp -- 2>/dev/null \
  | python3 -c "
import sys, json
for line in sys.stdin:
    try:
        data = json.loads(line)
        if data.get('id') == 2:
            text = data['result']['content'][0]['text']
            parsed = json.loads(text)
            required = ['node_count','edge_count','languages','communities',
                        'hotspots','entry_points','god_nodes','indexed_at']
            missing = [k for k in required if k not in parsed]
            if missing:
                print('FAIL — summary missing fields:', missing)
            else:
                print('PASS — get_repo_summary returned all fields')
                print('  node_count:', parsed['node_count'])
                print('  communities:', len(parsed['communities']))
                print('  hotspots:', len(parsed['hotspots']))
    except Exception as e:
        print('ERROR:', e)
"
```
**Expected:** `PASS — get_repo_summary returned all fields`, node_count > 0

---

## Phase 8B — Skills System Verification

> These run after `cgx analyze` completes on any fixture repo.
> The skills system is part of Phase 8 — do not skip these.

### Scenario 8B.1 — `CGX_SKILL.md` is created after analyze
```bash
cargo run -p cgx-cli -- analyze tests/fixtures/ts-sample 2>/dev/null
ls -la tests/fixtures/ts-sample/CGX_SKILL.md 2>&1
```
**Expected:** File exists. Size > 500 bytes. Not empty.

### Scenario 8B.2 — `CGX_SKILL.md` contains all required sections
```bash
python3 -c "
with open('tests/fixtures/ts-sample/CGX_SKILL.md') as f:
    content = f.read()

required_sections = [
    '## When to Use cgx',
    '## Trigger Patterns',
    '## Commands',
    '## Workflow',
    '## Token Budget',
    '## This Codebase',
    'cgx summary',
    'cgx query find',
    'cgx query blast-radius',
    'cgx hotspots',
]
missing = [s for s in required_sections if s not in content]
if missing:
    print('FAIL — missing sections:', missing)
else:
    print('PASS — all required sections present')
"
```
**Expected:** `PASS — all required sections present`

### Scenario 8B.3 — `CGX_SKILL.md` placeholder tokens are all replaced
```bash
python3 -c "
import re
with open('tests/fixtures/ts-sample/CGX_SKILL.md') as f:
    content = f.read()

unfilled = re.findall(r'\{\{[^}]+\}\}', content)
if unfilled:
    print('FAIL — unfilled placeholders found:', unfilled)
else:
    print('PASS — no unfilled placeholders')
"
```
**Expected:** `PASS — no unfilled placeholders`

### Scenario 8B.4 — `CGX_SKILL.md` codebase stats are accurate
```bash
python3 -c "
import re
with open('tests/fixtures/ts-sample/CGX_SKILL.md') as f:
    content = f.read()

# Node count line should have a real number
node_match = re.search(r'Nodes:\s*\*\*(\d+)', content)
if not node_match:
    print('FAIL — could not find node count in skill file')
else:
    count = int(node_match.group(1))
    if count < 10:
        print(f'FAIL — node count looks wrong: {count} (expected >= 10 for ts-sample)')
    else:
        print(f'PASS — node count is {count}')

# Indexed date should exist and look like a real date
date_match = re.search(r'Indexed:\s*\*\*([\d\-T:Z]+)', content)
if not date_match:
    print('FAIL — no indexed date found in skill file')
else:
    print(f'PASS — indexed date: {date_match.group(1)}')
"
```
**Expected:** Both `PASS` lines. Node count >= 10 for ts-sample.

### Scenario 8B.5 — `CGX_SKILL.md` hotspots list is populated
```bash
# Use git-sample which has real churn data
cargo run -p cgx-cli -- analyze tests/fixtures/git-sample 2>/dev/null
python3 -c "
with open('tests/fixtures/git-sample/CGX_SKILL.md') as f:
    content = f.read()

# Find hotspots section
if '### Hotspots' not in content:
    print('FAIL — no Hotspots section in skill')
else:
    # Extract hotspots section
    start = content.index('### Hotspots')
    section = content[start:start+500]
    # Should mention auth.ts (highest churn in git-sample)
    if 'auth' in section.lower() or 'db' in section.lower():
        print('PASS — hotspots section mentions high-churn files')
        print(section[:200])
    else:
        print('WARN — hotspots section exists but expected files not listed')
        print(section[:200])
"
```
**Expected:** `PASS — hotspots section mentions high-churn files`

### Scenario 8B.6 — `AGENTS.md` is created after analyze
```bash
ls -la tests/fixtures/ts-sample/AGENTS.md 2>&1
python3 -c "
with open('tests/fixtures/ts-sample/AGENTS.md') as f:
    content = f.read()

required = ['## Overview', '## Module Map', '## Hotspots',
            '## Entry Points', '## AI Integration', 'CGX_SKILL.md']
missing = [s for s in required if s not in content]
if missing:
    print('FAIL — AGENTS.md missing sections:', missing)
else:
    print('PASS — AGENTS.md has all required sections')
    print('Size:', len(content), 'chars')
"
```
**Expected:** `PASS — AGENTS.md has all required sections`

### Scenario 8B.7 — `AGENTS.md` does not contain unfilled placeholders
```bash
python3 -c "
import re
with open('tests/fixtures/ts-sample/AGENTS.md') as f:
    content = f.read()
unfilled = re.findall(r'\{\{[^}]+\}\}', content)
if unfilled:
    print('FAIL — unfilled placeholders:', unfilled)
else:
    print('PASS — no unfilled placeholders in AGENTS.md')
"
```
**Expected:** `PASS — no unfilled placeholders in AGENTS.md`

### Scenario 8B.8 — Git hooks are installed after analyze
```bash
# Use git-sample (a real git repo)
cargo run -p cgx-cli -- analyze tests/fixtures/git-sample 2>/dev/null
ls -la tests/fixtures/git-sample/.git/hooks/post-commit 2>&1
ls -la tests/fixtures/git-sample/.git/hooks/post-checkout 2>&1
```
**Expected:** Both hook files exist. Both are listed without error.

```bash
python3 -c "
import os, stat

for hook in ['post-commit', 'post-checkout']:
    path = 'tests/fixtures/git-sample/.git/hooks/' + hook
    if not os.path.exists(path):
        print(f'FAIL — {hook} does not exist')
        continue
    with open(path) as f:
        lines = f.readlines()
    if len(lines) < 2:
        print(f'FAIL — {hook} is too short')
        continue
    if '# cgx-managed' not in lines[1]:
        print(f'FAIL — {hook} missing cgx-managed marker on line 2')
        continue
    if 'cgx analyze' not in open(path).read():
        print(f'FAIL — {hook} does not call cgx analyze')
        continue
    mode = os.stat(path).st_mode
    if not (mode & stat.S_IXUSR):
        print(f'FAIL — {hook} is not executable')
        continue
    print(f'PASS — {hook}: exists, cgx-managed, executable')
"
```
**Expected:** Both hooks: `PASS — [hook]: exists, cgx-managed, executable`

### Scenario 8B.9 — Git hooks do not overwrite existing hooks
```bash
# Write a pre-existing hook manually (not cgx-managed)
mkdir -p /tmp/hook-test/.git/hooks
git init /tmp/hook-test 2>/dev/null
cat > /tmp/hook-test/.git/hooks/post-commit << 'EOF'
#!/bin/sh
echo "existing hook"
EOF
chmod +x /tmp/hook-test/.git/hooks/post-commit
echo "existing content" > /tmp/hook-test/index.ts

# Now run cgx analyze
cargo run -p cgx-cli -- analyze /tmp/hook-test 2>&1 | grep -i "hook\|skip\|exist" | head -5

# Verify original hook was NOT overwritten
content=$(cat /tmp/hook-test/.git/hooks/post-commit)
if echo "$content" | grep -q "existing hook"; then
    echo "PASS — pre-existing hook was preserved"
else
    echo "FAIL — pre-existing hook was overwritten"
fi
```
**Expected:**
- Output contains a warning about skipping or existing hook
- `PASS — pre-existing hook was preserved`

### Scenario 8B.10 — Incremental re-analyze updates skill file timestamp
```bash
cargo run -p cgx-cli -- analyze tests/fixtures/ts-sample 2>/dev/null
date1=$(python3 -c "
import re
with open('tests/fixtures/ts-sample/CGX_SKILL.md') as f: c = f.read()
m = re.search(r'Indexed:\s*\*\*([\S]+)', c)
print(m.group(1) if m else 'NOT_FOUND')
")

sleep 2

cargo run -p cgx-cli -- analyze tests/fixtures/ts-sample --force 2>/dev/null
date2=$(python3 -c "
import re
with open('tests/fixtures/ts-sample/CGX_SKILL.md') as f: c = f.read()
m = re.search(r'Indexed:\s*\*\*([\S]+)', c)
print(m.group(1) if m else 'NOT_FOUND')
")

if [ "$date1" != "$date2" ]; then
    echo "PASS — skill file timestamp updated after re-analyze"
    echo "  Before: $date1"
    echo "  After:  $date2"
else
    echo "FAIL — timestamp did not change after re-analyze"
    echo "  Both: $date1"
fi
```
**Expected:** `PASS — skill file timestamp updated after re-analyze`

### Scenario 8B.11 — Skill file token budget table is correct
```bash
python3 -c "
with open('tests/fixtures/ts-sample/CGX_SKILL.md') as f:
    content = f.read()

# Token budget table must exist and have correct comparison
if '## Token Budget' not in content:
    print('FAIL — no Token Budget section')
elif 'cgx summary' not in content:
    print('FAIL — cgx summary not in token budget table')
elif '2,000' in content or '15,000' in content:
    print('PASS — token budget table present with file cost comparison')
else:
    print('WARN — token budget section exists but file cost numbers not found')
"
```
**Expected:** `PASS — token budget table present with file cost comparison`

---

## Phase 9 — GitHub Pages Publisher Verification

> Requires a GitHub repo with push access. Use a test repo.

### Scenario 9.1 — Build step produces dist with injected data
```bash
cargo run -p cgx-cli -- publish --dry-run 2>&1
```
**Expected:** Shows what would be pushed. Lists files in dist/. Shows target URL.
Does NOT actually push in dry-run mode.

### Scenario 9.2 — Injected graph data is valid JSON in the HTML
```bash
cargo run -p cgx-cli -- publish --dry-run --out=/tmp/publish-test/ 2>/dev/null
python3 -c "
import re
with open('/tmp/publish-test/index.html') as f:
    html = f.read()
match = re.search(r'window\.__CGX_GRAPH__\s*=\s*(\{.*?\});', html, re.DOTALL)
if not match:
    print('FAIL — __CGX_GRAPH__ not found in HTML')
else:
    import json
    try:
        data = json.loads(match.group(1))
        print('PASS — graph data injected and valid JSON')
        print('Nodes:', len(data.get('nodes', [])))
    except Exception as e:
        print('FAIL — invalid JSON:', e)
"
```
**Expected:** `PASS — graph data injected and valid JSON`

### Scenario 9.3 — Published page loads without API calls (standalone)
After actually publishing (with a real GitHub repo), open the GitHub Pages URL and:
```
[ ] Page loads without network errors
[ ] Graph renders within 5 seconds
[ ] No requests to localhost:7373 in browser network tab
[ ] All data comes from window.__CGX_GRAPH__
[ ] Nodes are clickable and inspector works
```

---

## Phase 10 — Graph Diff Verification

### Scenario 10.1 — `cgx diff` shows changes between commits
```bash
cd tests/fixtures/git-sample
cargo run -p cgx-cli -- diff HEAD~2 2>&1
```
**Expected output contains:**
- `Added:` with node/edge counts > 0
- `Removed:` section (may be 0)
- At least one specific file or symbol name mentioned

### Scenario 10.2 — `cgx impact --since=Nd` shows downstream nodes
```bash
cd tests/fixtures/git-sample
cargo run -p cgx-cli -- impact --since=999d 2>&1
```
**Expected:** Shows at least one "Changed" file and at least one "Ripple" entry.
(using 999d to ensure all commits are included)

### Scenario 10.3 — Diff between identical commits is empty
```bash
cargo run -p cgx-cli -- diff HEAD 2>&1
```
**Expected:** Shows `0 added, 0 removed, 0 modified` or "no changes" message.

---

## Full Integration Test (Run After All Phases)

Save this as `scripts/integration-test.sh` and run it:

```bash
#!/usr/bin/env bash
set -e
PASS=0
FAIL=0

check() {
    local name="$1"
    local cmd="$2"
    local expected="$3"
    output=$(eval "$cmd" 2>&1)
    if echo "$output" | grep -q "$expected"; then
        echo "  ✓ $name"
        PASS=$((PASS + 1))
    else
        echo "  ✗ $name"
        echo "    Expected: $expected"
        echo "    Got: $(echo $output | head -c 200)"
        FAIL=$((FAIL + 1))
    fi
}

echo "=== CGX Integration Test ==="
echo ""

echo "[ Build ]"
check "cargo build" "cargo build --workspace 2>&1" "Finished"
check "npm build" "npm run build 2>&1" "built in"

echo ""
echo "[ Analysis Pipeline ]"
check "analyze ts-sample" "cargo run -p cgx-cli -- analyze tests/fixtures/ts-sample 2>&1" "Done"
check "status shows nodes" "cargo run -p cgx-cli -- status 2>&1" "Nodes:"
check "list shows repos" "cargo run -p cgx-cli -- list 2>&1" "ts-sample"

echo ""
echo "[ Git Layer ]"
check "analyze git-sample" "cargo run -p cgx-cli -- analyze tests/fixtures/git-sample 2>&1" "Done"
check "hotspots runs" "cargo run -p cgx-cli -- hotspots 2>&1" "Churn"
check "blame-graph runs" "cargo run -p cgx-cli -- blame-graph 2>&1" "alice"

echo ""
echo "[ Export ]"
check "json export" "cargo run -p cgx-cli -- export --format=json 2>/dev/null | python3 -c 'import sys,json; d=json.load(sys.stdin); print(\"nodes:\",len(d[\"nodes\"]))'" "nodes:"
check "mermaid export" "cargo run -p cgx-cli -- export --format=mermaid 2>/dev/null" "graph"
check "dot export" "cargo run -p cgx-cli -- export --format=dot 2>/dev/null" "digraph"

echo ""
echo "[ Skills System ]"
check "CGX_SKILL.md created" "ls tests/fixtures/ts-sample/CGX_SKILL.md" "CGX_SKILL"
check "no unfilled placeholders" "python3 -c \"import re; c=open('tests/fixtures/ts-sample/CGX_SKILL.md').read(); print('ok' if not re.findall(r'\{\{[^}]+\}\}', c) else 'PLACEHOLDERS')\"" "ok"
check "AGENTS.md created" "ls tests/fixtures/ts-sample/AGENTS.md" "AGENTS"
check "post-commit hook installed" "ls tests/fixtures/git-sample/.git/hooks/post-commit" "post-commit"
check "post-checkout hook installed" "ls tests/fixtures/git-sample/.git/hooks/post-checkout" "post-checkout"

echo ""
echo "[ MCP Server ]"
check "mcp initialize" "echo '{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{\"protocolVersion\":\"2024-11-05\",\"capabilities\":{},\"clientInfo\":{\"name\":\"test\",\"version\":\"1.0\"}}}' | timeout 3 cargo run -p cgx-mcp -- 2>/dev/null" "result"
check "mcp tools list" "(echo '{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{\"protocolVersion\":\"2024-11-05\",\"capabilities\":{},\"clientInfo\":{\"name\":\"test\",\"version\":\"1.0\"}}}'; echo '{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/list\",\"params\":{}}') | timeout 3 cargo run -p cgx-mcp -- 2>/dev/null" "get_repo_summary"

echo ""
echo "[ HTTP Server ]"
(timeout 5 cargo run -p cgx-cli -- serve 2>/dev/null &)
sleep 2
check "api graph endpoint" "curl -s http://localhost:7373/api/graph" "nodes"
check "api repos endpoint" "curl -s http://localhost:7373/api/repos" "name"
kill %1 2>/dev/null || true

echo ""
echo "[ Cargo Tests ]"
check "all unit tests" "cargo test --workspace 2>&1" "test result: ok"

echo ""
echo "================================"
echo "Results: $PASS passed, $FAIL failed"
if [ $FAIL -eq 0 ]; then
    echo "ALL TESTS PASSED ✓"
    exit 0
else
    echo "FAILURES DETECTED ✗"
    exit 1
fi
```

Run it:
```bash
chmod +x scripts/integration-test.sh
./scripts/integration-test.sh
```

**Expected:** `ALL TESTS PASSED ✓`

---

## Clippy — No Panics in Library Code

Run this before any release:
```bash
cargo clippy -p cgx-core -- \
  -D clippy::unwrap_used \
  -D clippy::expect_used \
  -D clippy::panic \
  -D clippy::todo \
  2>&1 | grep "^error" | wc -l
```
**Expected:** `0`

Library code must use `?` and `anyhow::Result` everywhere.
`unwrap()` and `expect()` are only permitted in:
- `tests/` directories
- `fn main()` in binary crates (top level only)
