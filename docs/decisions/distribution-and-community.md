# Distribution and community decisions

These records tell you how the project releases Post and works with other people. Read the [guide](README.md) first.

### D-113 License

**Decision.** Post has the Apache License, version 2.0.

**Reason.** This is a basic requirement of the project. The license lets each person use, change, and share the code.

### D-114 Source code host

**Decision.** The source code is in a public GitHub repository from the first commit.

**Reason.** Open work agrees with the open license.

**Not selected.**

- A private repository until a later date.

### D-115 Platforms

**Decision.** Post supports these systems equally:

- Windows 10 and Windows 11.
- macOS 12 and later.
- Linux with Wayland or X11.

Each system gets builds for x86-64 and ARM64 processors. The automatic checks run the tests on all three systems.

**Reason.** Equal support for the three systems is a basic requirement of the project.

**Not selected.**

- x86-64 only on Windows and Linux.
- The newest systems only.

### D-116 Packages

**Decision.** Version 1.0 offers these packages:

- Windows: an MSI installer and a portable zip file.
- macOS: one universal `.dmg` file for Apple silicon and Intel processors.
- Linux: Flatpak and AppImage.
- Server: static programs for the three systems, and a container image.

**Reason.** This set is sufficient for almost all users, and we can test each package well.

**Not selected.**

- Archive files only.
- A larger set with many package managers.

### D-117 Code signature

**Decision.** The 0.x builds have no code signature. The documents explain the warning that Windows and macOS show. Before version 1.0, we get an Apple Developer ID with notarization and a Windows code signature.

**Reason.** The cost starts only when the application is ready for many users.

**Not selected.**

- A code signature from the first build.
- No code signature at any time.

### D-118 Release builds

**Decision.** Each minor version tag starts a build of the application and the server for the three systems. The builds go into a GitHub release.

**Reason.** We find package problems on each system early.

**Not selected.**

- A build each night.
- Builds only near version 1.0.

### D-119 Application identifier

**Decision.** The identifier of the application is `io.github.collide2954.Post`. The home page of the project is the website (D-175).

**Reason.** The identifier is free and valid for Flatpak and macOS. An identifier is hard to change later.

**Not selected.**

- An identifier from a domain name of the project.

**History.** At first, the home page of the project was the GitHub repository. On 2026-11-21, the project added the website.

### D-120 Contributions

**Decision.** During the 0.x versions, bug reports and ideas are welcome as issues. For a code contribution, a short discussion with the project is necessary first. An external contributor signs off each commit (Developer Certificate of Origin). The project opens fully at version 1.0.

**Reason.** The design changes quickly during the 0.x versions. A review of a large contribution takes time from the design work.

**Not selected.**

- Open pull requests from the first day.
- No issues and no pull requests before version 1.0.

### D-121 Code of conduct

**Decision.** The repository contains a short code of conduct that the project writes in ASD-STE100.

**Reason.** It agrees with the document rule and is easy to read for all people.

**Not selected.**

- The Contributor Covenant. Its text is not in ASD-STE100.
- No code of conduct before version 1.0.

### D-162 Security reports

**Date.** 2026-11-21.

**Decision.** A person reports a security problem with the private vulnerability report function of GitHub. The `SECURITY.md` file tells the person how to do this. The report stays private until a correction exists.

**Reason.** A private report protects the users. The project does not have to publish an email address.

**Not selected.**

- An email address.
- Public issues only.

### D-175 Website

**Date.** 2026-11-21.

**Decision.** The project has a website at `https://collide2954.github.io/post/`. GitHub Pages serves it.

- The home page tells what Post is. It has English and Latin American Spanish text.
- The website shows the documents of the repository as web pages, in English: the philosophy, the decision records, the protocol specification, the architecture, the threat model, and the changelog.
- A program in the repository makes the pages from the Markdown files. The automatic checks publish the website after each change.
- The website uses no scripts, no cookies, and no content from other servers.

**Reason.** A person who is not a programmer gets a clear first view of Post. Each document still has one source, thus the website cannot disagree with the repository. The website obeys the same privacy rule as the application (D-076).

**Not selected.**

- A home page only, with links to the documents in the repository.
- English text only.
- No website. This was the decision until 2026-11-21 (D-107, D-119).
