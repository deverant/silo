pub fn init_script(silo_bin: &str) -> String {
    format!(
        r##"# Silo shell integration for zsh
# Add to ~/.zshrc:
#   eval "$(silo shell init zsh)"

# Store path to real silo binary (before we shadow it with a function)
__silo_bin="{silo_bin}"

# Create session-scoped directive file for communication between binary and shell
export SILO_DIRECTIVE_FILE=$(mktemp)
trap "rm -f '$SILO_DIRECTIVE_FILE'" EXIT

silo() {{
    # Clear directive file before each command
    : > "$SILO_DIRECTIVE_FILE"

    # Run the silo binary
    "$__silo_bin" "$@"
    local exit_code=$?

    # Process directives from file
    while IFS='=' read -r key value; do
        case "$key" in
            cd) builtin cd "$value" ;;
            last)
                # Save current silo as "last" (for cd -), then update current
                export SILO_LAST="$SILO_CURRENT"
                export SILO_CURRENT="$value"
                ;;
        esac
    done < "$SILO_DIRECTIVE_FILE"

    return $exit_code
}}

# Completions
_silo() {{
    local -a completions

    # Pass all words after 'silo' to the completion command
    # words[2,-1] gets all elements from position 2 to the end
    completions=("${{(@f)$("$__silo_bin" shell complete-args -- "${{words[@]:1}}" 2>/dev/null)}}")
    [[ -n "$completions" ]] && _describe 'completion' completions
}}
compdef _silo silo
"##
    )
}
