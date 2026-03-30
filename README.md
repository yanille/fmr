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
- 💾 Memory-mapped git status caching for instant display
- 🚀 Lazy-loaded cache for optimal performance with large repo lists
- 🔄 `fmr upgrade` to upgrade the CLI
- ⬇️ `fmr downgrade <version>` to install an older release
- ♻️ `fmr refresh` to manage caches (repos, status, or both)

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

Status information is **cached for 5 minutes** to provide instant display. Use `fmr refresh status` to clear the cache and force fresh git checks.

---

### upgrade fmr

```bash
fmr upgrade
```

If installed in a system directory you may need:

```bash
sudo fmr upgrade
```

---

### Downgrade fmr

```bash
fmr downgrade 0.1.0
```

---

### Refresh Caches

`fmr` provides three refresh options:

#### Refresh repository list

```bash
fmr refresh list
# or
fmr refresh repos
```

This rescans your configured locations and rebuilds the repository index.

#### Clear git status cache

```bash
fmr refresh status
```

Clears the cached git status information, forcing fresh git checks on next display.

#### Refresh everything

```bash
fmr refresh all
```

Refreshes both the repository list and clears the status cache.

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

- Git status information is cached using a memory-mapped index for O(1) lookups:

```
~/.fmr/status_index.bin  # Path → offset mappings (loaded in memory)
~/.fmr/status_data.bin   # Binary status data (memory-mapped, lazy-loaded)
```

This architecture provides:
- **Fast searching** without rescanning every time
- **Instant status display** without repeated git commands
- **Lazy loading** - only reads status data when needed
- **Memory efficiency** - minimal RAM usage even with thousands of repos

---

## Repository

https://github.com/yanille/fmr
