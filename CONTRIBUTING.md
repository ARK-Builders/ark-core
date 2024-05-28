You made it here! This is a great step in helping to contribute to ARK 🎈

## How to contribute

To get started, you can start off here [issues](https://github.com/ARK-Builders/ark-rust/issues) with those tagged [`good first issue`](https://github.com/ARK-Builders/ark-rust/issues?q=is:issue+is:open+label:%22good+first+issue%22).

You can find fresh builds as artifacts of [GitHub Actions workflows](https://github.com/ARK-Builders/ark-rust/actions):

- The "Verify build" workflow runs tests on supported platforms
- Benchmarks are run on every PR. It uses [`criterion`](https://github.com/bheisler/criterion.rs) to measure performance of the code compared to current main branch

## Forking the project

Before we can add you as a contributor to our project, we suggest to do initial work from your own fork of the project.

To create a fork, please press `fork` button on the project page:
![fork](https://github.com/ARK-Builders/ark-rust/assets/60650661/fb950e9c-3bff-4850-9fa9-188dc59fdc15)

Then you can modify everything without fear of breaking official version.

## Submitting a Pull Request

After you've implemented a feature or fixed a bug, it is time to open Pull Request.
![pr](https://github.com/ARK-Builders/ark-rust/assets/60650661/ae6b0070-2d19-4c10-b09f-5f2b64e81c12)

Please enable GitHub Actions in your fork, so our QA will be able to download build of your version without manually compiling from source code.
![actions](https://github.com/ARK-Builders/ark-rust/assets/60650661/1ae7d5d3-30a4-4e19-8271-8a30ce1d4d99)

### Automated code style checks

The projects uses `rustfmt` and `clippy` to enforce code style and best practices. You can run them locally with:

```bash
cargo fmt --all
cargo clippy --workspace --bins -- -D warnings
```

### Code review

We care a lot about our software quality, that's why we are conducting strict code reviews before merging:

- we will ask questions if we are not sure about particular technical decision
- when possible, we will suggest alternative solution
- GitHub Actions workflow must result in success (be green)
- comments must be resolved before merge
- code style should be green as well

Right now, the team isn't that big, so please be patient 🙂

### Merge conflicts

If Pull Request is long time in reviewing phase, `main` branch might go forward too far.
Please, fix all merge conflicts in this case 🛠

## Additional read

https://docs.github.com/en/get-started/quickstart/github-flow
