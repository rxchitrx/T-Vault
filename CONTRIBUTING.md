# Contributing to T-Vault

Thank you for your interest in contributing to T-Vault! This document provides guidelines and instructions for contributing.

## Code of Conduct

- Be respectful and inclusive
- Focus on constructive feedback
- Help others learn and grow
- Follow the project's coding standards

## How to Contribute

### Reporting Bugs

1. Check if the bug has already been reported
2. Use the bug report template
3. Include:
   - Clear description of the issue
   - Steps to reproduce
   - Expected vs actual behavior
   - System information (macOS version, etc.)
   - Screenshots if applicable

### Suggesting Features

1. Check if the feature has been requested
2. Explain the use case and benefits
3. Provide examples if possible
4. Be open to discussion and feedback

### Pull Requests

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Make your changes
4. Test thoroughly
5. Commit with clear messages
6. Push to your fork
7. Open a Pull Request

## Development Setup

See [README.md](README.md) for detailed setup instructions.

## Coding Standards

### TypeScript/React

- Use functional components with hooks
- Follow React best practices
- Use TypeScript for type safety
- Keep components focused and small
- Use meaningful variable names

### Rust

- Follow Rust naming conventions
- Use `cargo fmt` for formatting
- Use `cargo clippy` for linting
- Handle errors properly (no unwrap in production code)
- Write tests for new functionality

### Git Commits

Use conventional commits:

- `feat:` New feature
- `fix:` Bug fix
- `docs:` Documentation changes
- `style:` Code style changes
- `refactor:` Code refactoring
- `test:` Adding tests
- `chore:` Maintenance tasks

Example:
```
feat: add batch file upload support

- Implement multi-file selection
- Add progress tracking
- Update UI with upload status
```

## Testing

### Before Submitting PR

- [ ] Code builds without errors
- [ ] All existing tests pass
- [ ] New features have tests
- [ ] UI works correctly
- [ ] No console errors
- [ ] Documentation updated

### Testing Checklist

- [ ] Login flow works
- [ ] File upload works
- [ ] File download works
- [ ] Folder creation works
- [ ] Gallery view works
- [ ] Settings persist
- [ ] Error handling works

## Project Structure

```
t-vault/
├── src/                 # React frontend
├── src-tauri/          # Rust backend
├── docs/               # Documentation
└── tests/              # Test files
```

## Need Help?

- Open a discussion on GitHub
- Ask in the issues section
- Check existing documentation

## License

By contributing, you agree that your contributions will be licensed under the MIT License.
