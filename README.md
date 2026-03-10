<p align="center">
  <img src="tauri/logo.svg" width="128" alt="FFBox Logo">
</p>

# FFBox

A lightweight, cross-platform graphical interface for FFmpeg batch conversions.

FFBox provides a UI around FFmpeg, making it easier to run batch media conversions without memorizing command-line arguments. It's built with Vue 3 on the frontend and Tauri (Rust) on the backend.

## Features

- **Dependency Management**: Checks for existing FFmpeg/FFprobe installations. If they aren't found, it downloads and extracts the correct binaries for your OvS to a local app folder.
- **Conversion Wizard**: A step-by-step UI to queue files, pick presets, and monitor progress.
- **Output Control**: Save converted files to a specific directory, or output them to an `FFBox` subfolder relative to where the source files are located.
- **Preset Manager**: Start with default FFmpeg presets, or add, edit, and delete your own command-line profiles through the settings panel.
- **Batch Processing**: Drag and drop individual files or entire folders. The app recursively finds media files and queues them.

## Tech Stack

- **Frontend**: Vue 3, Vite, Tailwind CSS
- **Backend**: Tauri, Rust
- **Engine**: FFmpeg

## Setup & Development

### Requirements
- Node.js (v16+)
- Rust

### Running Locally
To run the dev server with hot module replacement:

```bash
npm install
npm run tauri dev
```

### Build
To compile the standalone installer for your current OS:

```bash
npm run tauri build
```

The compiled binaries will be placed in `tauri/target/release/bundle/`.

## Configuration

FFBox stores configuration files in your home directory at `~/.ffbox/`:
- `settings.toml`: UI preferences, output locations, and executable paths.
- `presets.toml`: Conversion profiles and ffmpeg arguments.

