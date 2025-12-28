---
name: ai-guidelines
description: Use when the Rust code involves AI agents, LLM-driven code generation, making APIs easier for AI systems to use, comprehensive documentation and detailed examples, or strong type systems that help AI avoid mistakes
---

# AI Guidelines



## Design with AI use in Mind (M-DESIGN-FOR-AI) { #M-DESIGN-FOR-AI }

<why>To maximize the utility you get from letting agents work in your code base.</why>
<version>0.1</version>

As a general rule, making APIs easier to use for humans also makes them easier to use by AI.
If you follow the guidelines in this book, you should be in good shape.

Rust's strong type system is a boon for agents, as their lack of genuine understanding can often be
counterbalanced by comprehensive compiler checks, which Rust provides in abundance.

With that said, there are a few guidelines which are particularly important to help make AI coding in Rust more effective:

* **Create Idiomatic Rust API Patterns**. The more your APIs, whether public or internal, look and feel like the majority of
Rust code in the world, the better it is for AI. Follow the [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/checklist.html)
along with the guidelines from [Library / UX](../libs/ux).

* **Provide Thorough Docs**. Agents love good detailed docs. Include docs for all of your modules and public items in your crate.
Assume the reader has a solid, but not expert, level of understanding of Rust, and that the reader understands the standard library.
Follow
[C-CRATE-DOC](https://rust-lang.github.io/api-guidelines/checklist.html#c-crate-doc),
[C-FAILURE](https://rust-lang.github.io/api-guidelines/checklist.html#c-failure),
[C-LINK](https://rust-lang.github.io/api-guidelines/checklist.html#c-link), and
[M-MODULE-DOCS](../docs/#M-MODULE-DOCS)
[M-CANONICAL-DOCS](../docs/#M-CANONICAL-DOCS).

* **Provide Thorough Examples**. Your documentation should have directly usable examples, the repository should include more elaborate ones.
Follow
[C-EXAMPLE](https://rust-lang.github.io/api-guidelines/checklist.html#c-example)
[C-QUESTION-MARK](https://rust-lang.github.io/api-guidelines/checklist.html#c-question-mark).

* **Use Strong Types**. Avoid [primitive obsession](https://refactoring.guru/smells/primitive-obsession) by using strong types with strict well-documented semantics.
Follow
[C-NEWTYPE](https://rust-lang.github.io/api-guidelines/checklist.html#c-newtype).

* **Make Your APIs Testable**. Design APIs which allow your customers to test their use of your API in unit tests. This might involve introducing some mocks, fakes,
or cargo features. AI agents need to be able to iterate quickly to prove that the code they are writing that calls your API is working
correctly.

* **Ensure Test Coverage**. Your own code should have good test coverage over observable behavior.
This enables agents to work in a mostly hands-off mode when refactoring.




