document.addEventListener("DOMContentLoaded", () => {
    const nav = document.getElementById("docs-nav");
    if (!nav) {
        return;
    }

    const links = Array.from(nav.querySelectorAll(".nav__item"));
    if (!links.length) {
        return;
    }

    const sections = links
        .map((link) => {
            const id = link.dataset.section;
            if (!id) {
                return null;
            }
            const section = document.getElementById(id);
            return section ? { id, section } : null;
        })
        .filter(Boolean);

    const activateLink = (id) => {
        links.forEach((link) => {
            if (link.dataset.section === id) {
                link.classList.add("nav__item--active");
            } else {
                link.classList.remove("nav__item--active");
            }
        });
    };

    const observer = new IntersectionObserver(
        (entries) => {
            const visible = entries
                .filter((entry) => entry.isIntersecting)
                .sort((a, b) => a.target.offsetTop - b.target.offsetTop);

            if (visible.length > 0) {
                const id = visible[0].target.id;
                activateLink(id);
            }
        },
        {
            rootMargin: "-40% 0px -55% 0px",
            threshold: 0.1,
        }
    );

    sections.forEach(({ section }) => observer.observe(section));

    links.forEach((link) => {
        link.addEventListener("click", () => {
            const id = link.dataset.section;
            if (id) {
                activateLink(id);
            }
        });
    });

    window.addEventListener("hashchange", () => {
        const hash = window.location.hash.replace("#", "");
        if (hash) {
            activateLink(hash);
        }
    });

    // Ensure the correct link is active on initial load with hash
    if (window.location.hash) {
        const hash = window.location.hash.replace("#", "");
        activateLink(hash);
    } else if (sections.length > 0) {
        activateLink(sections[0].id);
    }
});
