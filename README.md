# fmr

**Find My Repo** — a fast CLI for finding and opening your local Git repositories.

`fmr` scans your system, builds a cache of repositories, and lets you quickly search and open them in **VS Code**.

---

## Features

- 🔎 Search your local Git repositories
- 📂 Open repos directly in **VS Code**
- ⚡ Cached repo index for fast lookups
- 🔄 `fmr update` to update the CLI
- ⬇️ `fmr downgrade <version>` to install an older release
- ♻️ `fmr refresh` to rebuild the repository cache

---

## Usage

Search for a repository:

```bash
fmr my-repo
```

If multiple matches are found, you'll be prompted to select one.

---

### Update fmr

```bash
fmr update
```

If installed in a system directory you may need:

```bash
sudo fmr update
```

---

### Downgrade fmr

```bash
fmr downgrade 0.1.0
```

---

### Refresh the repo cache

```bash
fmr refresh
```

This rescans your system and rebuilds the repository index.

---

## How It Works

- `fmr` scans your **Desktop directory** for folders containing `.git`.
- Repositories are cached in:

```
~/.fmr/repos.json
```

This allows fast searching without rescanning every time.

---

## Repository

https://github.com/yanille/fmr