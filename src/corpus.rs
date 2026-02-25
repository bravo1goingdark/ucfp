//! Sample corpus data for the main binary.
//! This module contains sample documents for testing and benchmarking.

/// Tenant names for the corpus.
pub const TENANTS: [&str; 4] = ["tenant-a", "tenant-b", "tenant-c", "tenant-d"];

/// Programming language documents.
pub const PROGRAMMING_DOCS: [&str; 20] = [
    "Rust is a systems programming language that runs blazingly fast, prevents segfaults, and guarantees thread safety. Ownership and borrowing are core concepts.",
    "Python is a high-level, interpreted programming language with dynamic typing and automatic memory management. Great for rapid prototyping.",
    "Go is statically typed, compiled programming language designed at Google. It is syntactically similar to C, but with memory safety, garbage collection, structural typing, and CSP-style concurrency.",
    "JavaScript is the programming language of the Web. It is lightweight, interpreted, and just-in-time compiled programming language with first-class functions.",
    "TypeScript is a strongly typed programming language that builds on JavaScript, giving you better tooling at any scale. It adds static type checking.",
    "C++ is a high-performance programming language used for system programming, game development, and embedded systems. It provides low-level memory manipulation.",
    "Java is a class-based, object-oriented programming language designed to have as few implementation dependencies as possible. Write once, run anywhere.",
    "Kotlin is a cross-platform, statically typed, general-purpose programming language with type inference. It is designed to interoperate fully with Java.",
    "Swift is a powerful and intuitive programming language for iOS, iPadOS, macOS, tvOS, and watchOS. It is designed to work with Apple's Cocoa frameworks.",
    "Zig is a general-purpose programming language and build system that aims to be robust, optimal, and reusable. It focuses on explicit control over memory.",
    "Elixir is a dynamic, functional language designed for building scalable and maintainable applications. It runs on the Erlang Virtual Machine.",
    "Haskell is a general-purpose, statically typed, purely functional programming language with type inference and lazy evaluation. Great for mathematical abstractions.",
    "Clojure is a modern, dynamic, and functional dialect of the Lisp programming language on the Java platform. It emphasizes immutable data structures.",
    "Scala combines object-oriented and functional programming in one concise, high-level language. It runs on the JVM and can interoperate with Java.",
    "Ruby is a dynamic, open source programming language with a focus on simplicity and productivity. It has an elegant syntax that is natural to read.",
    "PHP is a popular general-purpose scripting language that is especially suited to web development. It was created by Rasmus Lerdorf in 1994.",
    "Perl is a family of high-level, general-purpose, interpreted, dynamic programming languages. Known for its text processing capabilities and regex.",
    "Lua is a lightweight, high-level, multi-paradigm programming language designed primarily for embedded use in applications. It is cross-platform.",
    "R is a programming language and free software environment for statistical computing and graphics. It is widely used among statisticians and data miners.",
    "MATLAB is a proprietary multi-paradigm programming language and numeric computing environment. It allows matrix manipulations, plotting of functions and data.",
];

/// Computer science documents.
pub const CS_DOCS: [&str; 20] = [
    "Machine learning is a branch of artificial intelligence focused on building applications that learn from data and improve their accuracy over time without being programmed.",
    "Deep learning is part of a broader family of machine learning methods based on artificial neural networks with representation learning. It uses multiple layers.",
    "Blockchain is a system of recording information in a way that makes it difficult or impossible to change, hack, or cheat the system. It is a digital ledger.",
    "Cloud computing is the delivery of computing services including servers, storage, databases, networking, software, and analytics over the Internet.",
    "Cybersecurity is the practice of protecting systems, networks, and programs from digital attacks. These cyberattacks are usually aimed at accessing sensitive data.",
    "Big Data refers to data that is so large, fast or complex that it is difficult or impossible to process using traditional methods. Volume, velocity, variety.",
    "Internet of Things describes the network of physical objects embedded with sensors, software, and other technologies for the purpose of connecting and exchanging data.",
    "Virtual Reality is a simulated experience that can be similar to or completely different from the real world. Applications include entertainment and education.",
    "Artificial Intelligence is the simulation of human intelligence processes by machines, especially computer systems. These processes include learning and reasoning.",
    "Natural Language Processing is a branch of AI that helps computers understand, interpret and manipulate human language. It bridges the gap between humans and machines.",
    "Computer Vision is a field of artificial intelligence that trains computers to interpret and understand the visual world using digital images and deep learning models.",
    "Distributed Systems are computing systems whose components are located on different networked computers, which communicate and coordinate their actions by passing messages.",
    "Microservices architecture structures an application as a collection of loosely coupled services. It enables the continuous delivery and deployment of large complex applications.",
    "Containerization is a lightweight alternative to full machine virtualization that involves encapsulating an application in a container with its own operating environment.",
    "DevOps is a set of practices that combines software development and IT operations. It aims to shorten the systems development life cycle and provide continuous delivery.",
    "Kubernetes is an open-source container-orchestration system for automating computer application deployment, scaling, and management. It was originally designed by Google.",
    "Docker is a set of platform as a service products that use OS-level virtualization to deliver software in packages called containers. It isolates applications.",
    "GraphQL is a query language for APIs and a runtime for executing those queries with your existing data. It provides a complete and understandable description of the data.",
    "REST API is an architectural style for an application program interface that uses HTTP requests to access and use data. GET, PUT, POST and DELETE.",
    "WebSocket is a computer communications protocol, providing full-duplex communication channels over a single TCP connection. It enables real-time data transfer.",
];

/// Science documents.
pub const SCIENCE_DOCS: [&str; 20] = [
    "Quantum mechanics is a fundamental theory in physics that provides a description of the physical properties of nature at the scale of atoms and subatomic particles.",
    "General relativity is the geometric theory of gravitation published by Albert Einstein in 1915. It describes gravity as a geometric property of space and time.",
    "Climate change refers to long-term shifts in temperatures and weather patterns. These shifts may be natural, but since the 1800s human activities have been the main driver.",
    "Photosynthesis is the process by which green plants and some other organisms use sunlight to synthesize foods with the help of chlorophyll pigment. It converts CO2.",
    "DNA is a molecule composed of two polynucleotide chains that coil around each other to form a double helix carrying genetic instructions for development and functioning.",
    "Evolution is change in the heritable characteristics of biological populations over successive generations. It is a fundamental concept in biology.",
    "The immune system is a network of biological processes that protects an organism from diseases. It detects and responds to a wide variety of pathogens.",
    "Neuroscience is the scientific study of the nervous system. It combines physiology, anatomy, molecular biology, developmental biology, and psychology.",
    "CRISPR is a family of DNA sequences found in the genomes of prokaryotic organisms. It is a technology that can be used to edit genes within organisms.",
    "Renewable energy is energy derived from natural sources that are replenished at a higher rate than they are consumed. Examples include sunlight and wind.",
    "Artificial neural networks are computing systems inspired by the biological neural networks that constitute animal brains. They learn to perform tasks by considering examples.",
    "The human genome is a complete set of nucleic acid sequences for humans, encoded as DNA within the 23 chromosome pairs in cell nuclei and in a small DNA molecule.",
    "Antibiotics are medicines that fight bacterial infections in people and animals. They work by killing the bacteria or making it hard for the bacteria to grow.",
    "Vaccines are products that protect people against serious diseases. They work by preparing the body's immune system to recognize and fight off infections.",
    "Black holes are regions of spacetime where gravity is so strong that nothing, including light or other electromagnetic waves, has enough energy to escape.",
    "Dark matter is a hypothetical form of matter thought to account for approximately 85 percent of the matter in the universe. It is invisible to telescopes.",
    "The Big Bang theory is the prevailing cosmological model explaining the existence of the observable universe from the earliest known periods through its subsequent evolution.",
    "Plate tectonics is a scientific theory describing the large-scale motion of seven large plates and the movements of a larger number of smaller plates of the Earth.",
    "Volcanoes are ruptures in the crust of a planetary-mass object, such as Earth, that allow hot lava, volcanic ash, and gases to escape from a magma chamber.",
    "Earthquakes are the shaking of the surface of the Earth resulting from a sudden release of energy in the Earth's lithosphere that creates seismic waves.",
];

/// Business documents.
pub const BUSINESS_DOCS: [&str; 20] = [
    "Cryptocurrency is a digital or virtual currency that is secured by cryptography, which makes it nearly impossible to counterfeit or double-spend. Bitcoin is the first.",
    "Stock market refers to the collection of markets and exchanges where regular activities of buying, selling, and issuance of shares of publicly-held companies take place.",
    "Investment banking is a specific division of banking related to the creation of capital for other companies, governments and other entities. It underwrites new debt.",
    "Venture capital is a form of private equity and a type of financing that investors provide to startup companies and small businesses that are believed to have long-term potential.",
    "Marketing is the activity, set of institutions, and processes for creating, communicating, delivering, and exchanging offerings that have value for customers and society.",
    "Supply chain management is the management of the flow of goods and services and includes all processes that transform raw materials into final products.",
    "Human resources is the division of a business responsible for finding, screening, recruiting, and training job applicants, and administering employee-benefit programs.",
    "Project management is the process of leading the work of a team to achieve all project goals within the given constraints. This information is usually described in documentation.",
    "Customer relationship management is the combination of practices, strategies and technologies that companies use to manage and analyze customer interactions and data.",
    "E-commerce is the buying and selling of goods or services using the internet, and the transfer of money and data to execute these transactions.",
    "Digital marketing is the component of marketing that uses internet and online-based digital technologies such as desktop computers, mobile phones and other digital media.",
    "Data analytics is the science of analyzing raw data to make conclusions about that information. Many of the techniques and processes of data analytics have been automated.",
    "Risk management is the identification, evaluation, and prioritization of risks followed by coordinated and economical application of resources to minimize and control the probability.",
    "Corporate governance is the collection of mechanisms, processes and relations used by various parties to control and to operate a corporation. It balances stakeholder interests.",
    "Intellectual property is a category of property that includes intangible creations of the human intellect. There are many types of intellectual property, including patents and copyrights.",
    "Mergers and acquisitions are transactions in which the ownership of companies, other business organizations, or their operating units are transferred or consolidated with other entities.",
    "Financial statements are formal records of the financial activities and position of a business, person, or other entity. They include balance sheets and income statements.",
    "Economics is a social science that studies how individuals, governments, firms, and nations make choices about allocating limited resources to satisfy their unlimited wants.",
    "Monetary policy is the policy adopted by the monetary authority of a nation to control either the interest rate payable for very short-term borrowing or the money supply.",
    "Inflation is a decrease in the purchasing power of money, reflected in a general increase in the prices of goods and services in an economy over time.",
];

/// Philosophy documents.
pub const PHILOSOPHY_DOCS: [&str; 20] = [
    "Existentialism is a form of philosophical inquiry that explores the problem of human existence and centers on the subjective experience of thinking and feeling.",
    "Stoicism is a school of Hellenistic philosophy founded by Zeno of Citium in Athens in the early 3rd century BC. It teaches self-control and fortitude.",
    "Plato was a philosopher in Classical Greece and the founder of the Academy in Athens, the first institution of higher learning in the Western world.",
    "Aristotle was a Greek philosopher and polymath during the Classical period in Ancient Greece. He was the founder of the Lyceum and the Peripatetic school.",
    "Immanuel Kant was a German philosopher and one of the central Enlightenment thinkers. He synthesized early modern rationalism and empiricism and set the terms for much of nineteenth and twentieth century philosophy.",
    "Friedrich Nietzsche was a German philosopher, cultural critic, composer, poet, writer, and philologist whose work has exerted a profound influence on modern intellectual history.",
    "Jean-Paul Sartre was a French philosopher, playwright, novelist, screenwriter, political activist, biographer, and literary critic. He was one of the key figures in the philosophy of existentialism.",
    "Simone de Beauvoir was a French existentialist philosopher, writer, social theorist, and feminist activist. She wrote The Second Sex, a detailed analysis of women's oppression.",
    "Karl Marx was a German philosopher, economist, historian, sociologist, political theorist, journalist, and socialist revolutionary. His best-known titles are the Communist Manifesto.",
    "Adam Smith was a Scottish economist and philosopher who was a pioneer of political economy and a key figure during the Scottish Enlightenment. He wrote Wealth of Nations.",
    "John Locke was an English philosopher and physician, widely regarded as one of the most influential of Enlightenment thinkers and commonly known as the Father of Liberalism.",
    "Rene Descartes was a French philosopher, mathematician, and scientist who invented analytic geometry, linking the previously separate fields of geometry and algebra.",
    "Socrates was a Greek philosopher from Athens who is credited as the founder of Western philosophy and as among the first moral philosophers of the ethical tradition.",
    "Epicurus was an ancient Greek philosopher and sage who founded Epicureanism, a highly influential school of philosophy. He taught that pleasure is the greatest good.",
    "Confucius was a Chinese philosopher and politician of the Spring and Autumn period who was traditionally considered the paragon of Chinese sages. He taught moral values.",
    "Lao Tzu was an ancient Chinese philosopher and writer. He is the reputed author of the Tao Te Ching, the founder of philosophical Taoism, and a deity in religious Taoism.",
    "Buddhism is a major world religion and philosophical system founded in northeastern India in the 5th century BC based on the teachings of Siddhartha Gautama, known as Buddha.",
    "Hinduism is an Indian religion and dharma, or way of life, widely practiced in the Indian subcontinent and parts of Southeast Asia. It is the world's third-largest religion.",
    "Christianity is an Abrahamic monotheistic religion based on the life and teachings of Jesus of Nazareth. It is the world's largest religion with about 2.4 billion followers.",
    "Islam is an Abrahamic monotheistic religion teaching that Muhammad is a messenger of God. It is the world's second-largest religion with 1.9 billion followers.",
];

/// Generate a large corpus of documents from all categories.
pub fn generate_large_corpus() -> Vec<(&'static str, String, String)> {
    let num_tenants = TENANTS.len();

    let all_docs: Vec<&str> = PROGRAMMING_DOCS
        .iter()
        .chain(CS_DOCS.iter())
        .chain(SCIENCE_DOCS.iter())
        .chain(BUSINESS_DOCS.iter())
        .chain(PHILOSOPHY_DOCS.iter())
        .copied()
        .collect();

    let total_items = all_docs.len() * num_tenants * 3;
    let mut corpus = Vec::with_capacity(total_items);

    for (doc_idx, doc) in all_docs.iter().enumerate() {
        for (tenant_idx, tenant) in TENANTS.iter().enumerate() {
            for variant in 0..3 {
                let doc_id = format!("doc-{:04}-{:02}", doc_idx, tenant_idx * 3 + variant);
                let text = match variant {
                    0 | 1 => doc.to_string(),
                    2 => format!("{} [v{}]", doc, variant),
                    _ => doc.to_string(),
                };
                corpus.push((*tenant, doc_id, text));
            }
        }
    }

    corpus.sort_by(|a, b| a.1.cmp(&b.1));
    corpus
}
