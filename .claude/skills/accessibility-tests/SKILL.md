---
name: accessibility-tests
description: Writes accessibility tests. Use when user asks to write a11y tests, accessibility tests, or test for screen readers, keyboard navigation, ARIA, or WCAG compliance.
---

# General approach

- Fully analyze code under test to understand what aspects needs to be tested
- Read WCAG quickref to find related success criteria: https://www.w3.org/WAI/WCAG22/quickref/
- Proceed as previously defined testing standards/best practices/instructions

# Must

- **Read WCAG quickref** - always read WCAG quickref to find what has to be tested
- **Always reference WCAG success criteria** - test only success criteria defined by WCAG and reference it in the test
- **Always reference WCAG technique** - test application of corresponding technique used to met given success criteria
- **Use Criteria -> Technique patters** - use WCAG quickref titles (copy-paste) to follow such structure:

```js
 describe("WCAG", () => {
    describe("1.3.1 Info and Relationships", () => {
      describe("ARIA1: Using the aria-describedby property to provide a descriptive label for user interface controls", () => {
        // multiple things to test this technique
        it("links input to hint text via aria-describedby", () => { ... });
        it("links input to error message via aria-describedby", async () => { ... });
      });
    });

    describe("3.3.1 Error Identification", () => {
      // simple single test, no need to nest it more
      it("ARIA21: Using aria-invalid to indicate an error field", () => {...})
    })
 })
```

- **Suggest mist criteria** - suggest missed success criteria that where not implemented but analyzed code seams to match it.

# Avoid

- **Using technique names that don't match what the test verifies** - The WCAG technique in the `describe` block must accurately describe what the test asserts. Don't use "ARIA1: Using aria-describedby..." if the test checks `aria-controls`. If no specific technique exists for what you're testing, either find a broader matching technique or group it under one that covers the same concept.
