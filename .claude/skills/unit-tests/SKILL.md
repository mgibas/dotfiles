---
name: unit-testing
description: Writes unit tests. Use when user asks to cover given code with unit tests
---

# General Approach

When user asks to cover given code with unit tests use these steps:

- Fully analyze code under test, you need to understand every branch
- Analyze boundaries - check dependencies and decide what has to be mocked/stubbed out
- Divide functionality into distinct aspects/groups - depending on the code under test, some will concern parsing user input, some validation etc.
- Present grouped test cases to the user before proceeding
- write tests
- run tests and iterate till they're green

# Musts

- **Use existing tools and libraries** - check what tools are used in current codebase and use the same
- **Arrange, Act, Assert** - write every test in AAA structure
- **Test behavior, not implementation** - Test what users see and do
- **One assertion focus per test** - Each test should verify one behavior
- **Descriptive test names** - Use "it does X when Y" format
- **Avoid testing implementation details** - Don't test internal state
- **Keep tests independent** - Tests should not depend on each other
- **Use test cases mechanism** - Use available mechanism like `each` to test same thing for different input
- **Write accessibility tests** - If you're testing front-end code, always consider writing accessibility tests

# Avoid

- Testing internal component state directly
- Using arbitrary `waitFor` delays
- Testing style properties unless visually critical
- Mocking everything (only mock boundaries)
- Testing third-party library behavior
