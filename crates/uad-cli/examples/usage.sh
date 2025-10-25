#!/usr/bin/env bash

# Example usage script for UAD CLI

# Check if uad-cli is available
if ! command -v uad &> /dev/null; then
    echo "uad command not found. Please build and install uad-cli first."
    exit 1
fi

echo "=== UAD CLI Usage Examples ==="
echo

echo "1. List connected devices:"
uad devices
echo

echo "2. List all enabled packages:"
uad list --state enabled | head -20
echo "... (showing first 20)"
echo

echo "3. Search for Google packages:"
uad list --search google | head -10
echo "... (showing first 10)"
echo

echo "4. Show info about a package (example):"
echo "uad info com.google.android.gms"
echo

echo "5. Dry-run uninstall (safe to test):"
echo "uad uninstall com.example.package --dry-run"
echo

echo "6. Start interactive mode:"
echo "uad repl"
echo

echo "=== Generate shell completions ==="
echo "For bash: uad completions bash > ~/.local/share/bash-completion/completions/uad"
echo "For zsh:  uad completions zsh > ~/.local/share/zsh/site-functions/_uad"
echo "For fish: uad completions fish > ~/.config/fish/completions/uad.fish"

