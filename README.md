# fmr

**Find My Repo** — a fast CLI for finding and opening your local Git repositories.

`fmr` scans your system, builds a cache of repositories, and lets you quickly search and open them in **VS Code**.

---

## Features

- 🔎 Search your local Git repositories
- 📂 Open repos directly in **VS Code**
- ⚡ Parallel scanning with cached repo index for fast lookups
- 📍 Configure multiple scan locations
- 🟢 Visual status indicators (clean, behind remote, uncommitted changes)
- 🔄 `fmr update` to update the CLI
- ⬇️ `fmr downgrade <version>` to install an older release
- ♻️ `fmr refresh` to rebuild the repository cache

---

## Usage

Search for a repository:

```bash
fmr my-repo
```

If multiple matches are found, you'll be prompted to select one:

```
Select a repository (Ctrl+C to exit):
  > ● fmr
    ● my-project
    ● another-repo
```

---

### Repository Status Indicators

When selecting a repository, a colored circle indicates its status:

- **🟢 Green** — Repository is clean and up to date
- **🟠 Orange** — Repository is behind remote (needs `git pull`)
- **🔴 Red** — Repository has uncommitted changes

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

## Managing Scan Locations

By default, `fmr` scans your **Desktop directory**. You can configure additional locations:

### List configured locations

```bash
fmr locations list
```

### Add a new scan location

```bash
fmr locations add ~/projects
fmr locations add ~/work/repos
```

### Remove a scan location

```bash
fmr locations remove ~/projects
```

---

## How It Works

- `fmr` scans configured directories in **parallel** for folders containing `.git`
- Configuration is stored in:

```
~/.fmr/config.json
```

- Repositories are cached in:

```
~/.fmr/repos.json
```

This allows fast searching without rescanning every time.

---

## Repository

https://github.com/yanille/fmr