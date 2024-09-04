from crate_version import get_crate_version
import argparse
import re
import sys

def update_changelog(changelog_path, version):
    """Update the changelog by moving 'Unreleased' changes to a new version section."""
    with open(changelog_path, 'r', encoding='utf-8') as f:
        changelog = f.read()

    # Case-insensitive match for the 'Unreleased' section
    unreleased_pattern = re.compile(r'^# unreleased.*$', re.IGNORECASE | re.MULTILINE)
    match = unreleased_pattern.search(changelog)

    if not match:
        sys.stderr.write("Error: 'Unreleased' section not found.\n")
        sys.exit(1)

    # Extract content after "Unreleased" until the next section
    unreleased_start = match.end()
    next_section_match = re.search(r'^# ', changelog[unreleased_start:], re.MULTILINE)

    if next_section_match:
        unreleased_end = unreleased_start + next_section_match.start()
    else:
        unreleased_end = len(changelog)

    unreleased_content = changelog[unreleased_start:unreleased_end].strip()

    # Check if the version already exists in the changelog
    version_header = f"# v{version}"
    if version_header in changelog:
        sys.stderr.write(f"Error: Version {version} already exists in the changelog.\n")
        return

    # Insert the new version section after 'Unreleased'
    new_version_section = f"\n\n{version_header}\n\n{unreleased_content}\n"

    # Update the changelog
    updated_changelog = re.sub(unreleased_pattern, r"# Unreleased Changes\n", changelog)
    updated_changelog = updated_changelog[:unreleased_start] + new_version_section + updated_changelog[unreleased_end:]

    with open(changelog_path, 'w', encoding='utf-8') as f:
        f.write(updated_changelog)

    print(f"Changelog updated successfully for version {version}.")

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Update crate changelog by parsing version from Cargo manifest")
    parser.add_argument('crate_path', help="Path to the crate")
    
    args = parser.parse_args()

    # Get the crate version from Cargo.toml
    manifest_path = f'{args.crate_path}/Cargo.toml'
    crate_version = get_crate_version(manifest_path)
    
    # Update the changelog
    crate_changelog_path = f'{args.crate_path}/CHANGELOG.md'
    update_changelog(crate_changelog_path, crate_version)