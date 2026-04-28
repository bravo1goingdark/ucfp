<script lang="ts">
  import Nav from '$components/Nav.svelte';
  import Hero from '$components/Hero.svelte';
  import SocialProof from '$components/SocialProof.svelte';
  import Demo from '$components/Demo.svelte';
  import Usage from '$components/Usage.svelte';
  import Capabilities from '$components/Capabilities.svelte';
  import Inputs from '$components/Inputs.svelte';
  import HowItWorks from '$components/HowItWorks.svelte';
  import UseCases from '$components/UseCases.svelte';
  import Integrations from '$components/Integrations.svelte';
  import Pricing from '$components/Pricing.svelte';
  import Testimonials from '$components/Testimonials.svelte';
  import FAQ from '$components/FAQ.svelte';
  import FinalCTA from '$components/FinalCTA.svelte';
  import Footer from '$components/Footer.svelte';
  import Seo from '$lib/components/Seo.svelte';
  import { organizationJsonLd } from '$lib/seo';
</script>

<Seo
  title="UCFP — Universal Content Fingerprinting"
  description="Universal Content Fingerprinting — turn text, images, and audio into compact, comparable digests. Production-grade SDKs for Node, Python, and Rust."
  canonical="/"
  jsonLd={organizationJsonLd()}
/>

<Nav />
<Hero />
<SocialProof />
<Demo />
<Usage />
<Capabilities />
<Inputs />
<HowItWorks />
<UseCases />
<Integrations />
<Pricing />
<Testimonials />
<FAQ />
<FinalCTA />
<Footer />
