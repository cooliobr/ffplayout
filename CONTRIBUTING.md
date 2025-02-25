## Contribute to ffplayout

#### **Report a bug**

- Check issues if the bug was already reported.
- When this bug was not reported, please use the **bug report** template.
    * try to fill out every step
    * use code blocks for config, log and command line parameters
    * text from config and logging is preferred over screenshots

#### **Ask for help**

When something is not working, you can feel free to ask your question under issues. But please make some effort, so it makes it more easy to help. Please don't open issues in a "WhatsApp style", with only one line of text. As a general rule of thumb answer this points:

- what do you want to achieve?
- what have you already tried?
- have you looked at the help documents under [/docs/](/docs)?
- what exactly is not working?
- relevant logging output
- current configuration (ffplayout.yml)

#### **Feature request**

You can ask for features, but it can not be guaranteed that this will find its way to the code basis. Try to think if your idea is useful for others to and describe it in a understandable way. If your idea is accepted, it can take time until it will be apply. In general stability goes over features, and when just a new version has arrived, it can take time to prove itself in production.

#### **Create a pull request**

In general pull requests are very welcome! But please don't create features, which are to specific and helps only your use case and no one else. If your are not sure, better ask before you start.

Please also follow the code style from this project, and before you create your pull request check your code with:

```BASH
cargo fmt --all -- --check
cargo clippy --all-features --all-targets -- --deny warnings
```

For bigger changes and complied new functions a test is required.
