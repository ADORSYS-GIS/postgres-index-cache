# Commit Message Generator

## Purpose
Generate a well-formatted conventional commit message based on the current git changes and save it to `target/cm.md`.

## Instructions
1. Run `git status` and `git diff --staged` (or `git diff` if nothing is staged) to analyze changes
2. Identify the type and scope of changes:
   - **feat**: New feature
   - **fix**: Bug fix
   - **docs**: Documentation changes
   - **style**: Code style changes (formatting, missing semicolons, etc.)
   - **refactor**: Code refactoring without changing functionality
   - **test**: Adding or updating tests
   - **chore**: Maintenance tasks, dependency updates, etc.
   - **perf**: Performance improvements
3. Write a commit message following this format:
   ```
   <type>(<scope>): <short description>
   
   <optional detailed description>
   
   <optional footer for breaking changes or issue references>
   ```
4. Guidelines:
   - Use imperative mood ("add" not "added" or "adds")
   - Keep the first line under 72 characters
   - Capitalize the first letter of the description
   - No period at the end of the first line
   - Provide context in the body if the change is complex
   - Reference issues/tickets if applicable (e.g., "Closes #123")
5. Save the generated commit message to `target/cm.md`

## Example Output
```
feat(audit): add comprehensive tests for load_batch function

Implement 7 test cases covering:
- Empty input handling
- Single and multiple record loading
- Mixed existing/non-existent records
- Order preservation
- Duplicate ID handling

All tests use transactional isolation with automatic rollback.
```

## Usage
Simply reference this command: "Use `.roo/commands/cm.md`"