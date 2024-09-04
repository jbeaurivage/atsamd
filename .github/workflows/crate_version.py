import tomlkit
import sys
import argparse

def get_crate_version(manifest_path):
    # Read the Cargo.toml file
    with open(manifest_path, 'r', encoding='utf-8') as f:
        cargo_toml = tomlkit.parse(f.read())
    
    # Extract the version from the `[package]` section
    return cargo_toml["package"]["version"]


if __name__ == "__main__":
    # Set up argument parsing
    parser = argparse.ArgumentParser(description="Get crate version from a Cargo manifest file")
    parser.add_argument('manifest_path', help="Path to the Cargo.toml manifest")
    
    # Parse the arguments
    args = parser.parse_args()
    
    # Get the crate version
    try:
        version = get_crate_version(args.manifest_path)
        print(version)
    except (FileNotFoundError, KeyError):
        sys.stderr.write("Error: Could not find the version in the specified Cargo.toml file.\n")
        sys.exit(1)