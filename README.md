# Pulumi Profile Selector

A fast, interactive Pulumi profile selector CLI tool built in Rust.

## Features

- 🔍 **Fuzzy search** through Pulumi profiles
- ⚡ **Fast inline interface** - no full-screen takeover
- 🎯 **Arrow key navigation**
- 📦 **Single binary** with no runtime dependencies
- 🔧 **Profile management** - add, edit, and delete profiles

## Installation

### Option 1: Direct Installation with Nix Profile
```bash
# Install directly from GitHub
nix profile install github:stephenstubbs/pulumi-profile-selector

# Verify installation
pulumi-profile-selector --help
```

### Option 2: Run Without Installing
```bash
# Run directly from GitHub
nix run github:stephenstubbs/pulumi-profile-selector -- --help
```

### Option 3: Add to Your Flake
Add to `flake.nix` inputs:
```nix
{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

    pulumi-profile-selector = {
      url = "github:stephenstubbs/pulumi-profile-selector";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, pulumi-profile-selector, ... }:
    let
      system = "x86_64-linux";
      pkgs = nixpkgs.legacyPackages.${system};
    in
    {
      devShells.${system}.default = pkgs.mkShell {
        packages = [
          pulumi-profile-selector.packages.${system}.default
        ];
      };
    };
}
```

### Option 4: Development Environment
```bash
# Clone and enter development environment
git clone https://github.com/stephenstubbs/pulumi-profile-selector
cd pulumi-profile-selector
nix develop

# Build and run
cargo build --release
./target/release/pulumi-profile-selector --help
```

## Usage

### Direct Usage

Run the selector directly:
```bash
./target/release/pulumi-profile-selector
```

**Interactive Mode (default):**
```bash
pulumi-profile-selector                    # Interactive selection
```

**Direct Profile Activation:**
```bash
pulumi-profile-selector -a dev             # Activate 'dev' profile directly
pulumi-profile-selector --activate prod    # Activate 'prod' profile directly
```

**Set New Profile (not in profiles.json):**
```bash
pulumi-profile-selector -n custom          # Set 'custom' profile (even if not in profiles.json)
pulumi-profile-selector --new temp-profile # Set 'temp-profile' profile
```

**Deactivate Profile:**
```bash
pulumi-profile-selector -d                 # Deactivate PULUMI_BACKEND_URL
pulumi-profile-selector --deactivate       # Deactivate PULUMI_BACKEND_URL
```

**Set Profile for Current Shell Only:**
```bash
# For current shell session only (doesn't write to ~/.pulumi/current_profile)
pulumi-profile-selector -c                 # Interactive selection, outputs shell command
pulumi-profile-selector -c -a dev          # Outputs: $env.PULUMI_BACKEND_URL = "s3://..."
pulumi-profile-selector -c -n custom       # Outputs: $env.PULUMI_BACKEND_URL = "custom"
pulumi-profile-selector -c -d              # Outputs: hide-env PULUMI_BACKEND_URL
```

**Profile Management:**
```bash
pulumi-profile-selector --add              # Add a new profile interactively
pulumi-profile-selector --edit dev         # Edit 'dev' profile's backend URL
pulumi-profile-selector --delete old       # Delete 'old' profile
pulumi-profile-selector -l                 # List all profiles
pulumi-profile-selector --list             # List all profiles
```

**Options:**
- `-a, --activate <PROFILE>`: Activate a specific profile by name (skips interactive selection)
- `-n, --new <PROFILE>`: Set a profile name that is not available in the list
- `-c, --current`: Output shell commands for current shell only (doesn't write to file)
- `-d, --deactivate`: Deactivate PULUMI_BACKEND_URL
- `--add`: Add a new profile interactively
- `--edit <PROFILE>`: Edit an existing profile's backend URL
- `--delete <PROFILE>`: Delete a profile
- `-l, --list`: List all profiles

### Shell Integration (Nushell)

Add these functions and hooks to your nushell config (`~/.config/nushell/config.nu`):

#### Option 1: Using --wrapped flag (Recommended for Nushell 0.91.0+)
```nu
def --env --wrapped pulumips [...args] {
    # Check if -c or --current is in the arguments
    let is_current = ($args | any {|arg| $arg == "-c" or $arg == "--current"})

    if $is_current {
        # Interactive mode with -c: run and capture output to set env var
        let cmd = (^pulumi-profile-selector ...$args | str trim)

        if ($cmd | is-not-empty) {
            if ($cmd | str contains '$env.PULUMI_BACKEND_URL') {
                # Extract the backend URL from: $env.PULUMI_BACKEND_URL = "backend-url"
                let parts = ($cmd | parse '$env.PULUMI_BACKEND_URL = "{backend}"')
                if ($parts | length) > 0 {
                    let backend = ($parts | first | get backend)
                    $env.PULUMI_BACKEND_URL = $backend
                    $env.PULUMI_BACKEND_URL_CURRENT_SHELL = "true"
                    print $"PULUMI_BACKEND_URL set to ($backend) for current shell"
                }
            } else if ($cmd == 'hide-env PULUMI_BACKEND_URL') {
                hide-env PULUMI_BACKEND_URL
                if "PULUMI_BACKEND_URL_CURRENT_SHELL" in $env {
                    hide-env PULUMI_BACKEND_URL_CURRENT_SHELL
                }
                print "PULUMI_BACKEND_URL unset for current shell"
            }
        }
    } else {
        # Non-current mode: clear current shell flag and pass through
        if "PULUMI_BACKEND_URL_CURRENT_SHELL" in $env {
            hide-env PULUMI_BACKEND_URL_CURRENT_SHELL
        }
        ^pulumi-profile-selector ...$args
    }
}

# Usage examples:
# pulumips                      # Interactive selection (writes to file, hooks handle it)
# pulumips -a dev              # Activate 'dev' profile (writes to file, hooks handle it)
# pulumips -d                  # Deactivate profile (removes file, hooks handle it)
# pulumips -c                  # Interactive selection for current shell only
# pulumips -c -a dev           # Activate 'dev' for current shell only
# pulumips -c -d               # Deactivate for current shell only
# pulumips --current -n custom # Set custom profile for current shell only
# pulumips --help              # Show help for pulumi-profile-selector
# pulumips --add               # Add a new profile
# pulumips --edit dev          # Edit dev profile
# pulumips --delete old        # Delete old profile
# pulumips -l                  # List all profiles
```

**Note:** The `--wrapped` flag allows the function to receive all arguments without Nushell intercepting flags like `--help`. This provides seamless pass-through of all arguments to the underlying `pulumi-profile-selector` command.

#### Option 2: Persistent Profile (File-based)
```nu
def --env load_pulumi_profile [] {
    if "PULUMI_BACKEND_URL_CURRENT_SHELL" in $env {
        return
    }

    let current_profile_file = ([$env.HOME ".pulumi" "current_profile"] | path join)
    let profiles_file = ([$env.HOME ".pulumi" "profiles.json"] | path join)

    if ($current_profile_file | path exists) and ($profiles_file | path exists) {
        let profile_name = (open $current_profile_file | str trim)
        if ($profile_name | is-not-empty) {
            let profiles = (open $profiles_file)
            let profile = ($profiles | where name == $profile_name | first)
            if ($profile | is-not-empty) {
                $env.PULUMI_BACKEND_URL = $profile.backend
            }
        }
    } else {
        if "PULUMI_BACKEND_URL" in $env {
            hide-env PULUMI_BACKEND_URL
        }
    }
}

# Set up hooks to automatically load Pulumi profile
$env.config = ($env.config | upsert hooks {
    pre_prompt: [
        { ||
            load_pulumi_profile
        }
    ]
    env_change: {
        PWD: [
            { |before, after|
                load_pulumi_profile
            }
        ]
    }
})
```

This configuration will:
- **Automatically load the Pulumi profile** when nushell starts
- **Re-load the Pulumi profile** every time you change directories (`cd`)
- **Re-load the Pulumi profile** before each prompt is displayed
- **Provide the `pulumips` command** for interactive profile selection and management


## How It Works

1. **Reads your Pulumi profiles** from `~/.pulumi/profiles.json`
2. **Parses profile entries** and extracts name and backend URL
3. **Presents an interactive list** with fuzzy search capabilities
4. **Stores the selected profile** in `~/.pulumi/current_profile`
5. **Nushell integration** reads this file to set `$env.PULUMI_BACKEND_URL`

## Interface

- **↑/↓ arrows**: Navigate through profiles
- **Type**: Filter profiles with fuzzy search (no need to press `/`)
- **Enter**: Select the highlighted profile
- **Esc/q**: Cancel and exit

## Pulumi Profiles Format

The tool manages profiles in `~/.pulumi/profiles.json`. Example:

```json
[
  {
    "name": "dev",
    "backend": "s3://pulumi-state-dev"
  },
  {
    "name": "prod",
    "backend": "s3://pulumi-state-prod"
  },
  {
    "name": "local",
    "backend": "file://./state"
  }
]
```

## License

MIT License
