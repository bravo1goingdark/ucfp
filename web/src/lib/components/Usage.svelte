<script lang="ts">
  type Lang = 'ts' | 'py' | 'go' | 'curl';

  const SNIPPETS: Record<Lang, string> = {
    ts: `<span class="c">// Fingerprint anything.</span>
<span class="k">import</span> { fingerprint } <span class="k">from</span> <span class="s">"ucfp"</span>;

<span class="k">const</span> <span class="v">id</span> = <span class="k">await</span> <span class="fn">fingerprint</span>(input, {
  modality: <span class="s">"auto"</span>,    <span class="c">// text | image | audio | bytes</span>
  bits: <span class="v">256</span>,
  encoding: <span class="s">"multibase"</span>,
});

<span class="c">// → ucfp1·b3k4q7n2…wpx9</span>
<span class="fn">console</span>.<span class="fn">log</span>(id.toString());`,
    py: `<span class="c"># Fingerprint anything.</span>
<span class="k">from</span> ucfp <span class="k">import</span> fingerprint

id = <span class="fn">fingerprint</span>(
    input,
    modality=<span class="s">"auto"</span>,
    bits=<span class="v">256</span>,
    encoding=<span class="s">"multibase"</span>,
)

<span class="c"># → ucfp1·b3k4q7n2…wpx9</span>
<span class="fn">print</span>(id)`,
    go: `<span class="c">// Fingerprint anything.</span>
<span class="k">package</span> main

<span class="k">import</span> <span class="s">"github.com/ucfp/ucfp-go"</span>

<span class="k">func</span> main() {
    id, _ := ucfp.<span class="fn">Fingerprint</span>(input, ucfp.Opts{
        Modality: <span class="s">"auto"</span>,
        Bits:     <span class="v">256</span>,
        Encoding: <span class="s">"multibase"</span>,
    })
    <span class="c">// → ucfp1·b3k4q7n2…wpx9</span>
    fmt.<span class="fn">Println</span>(id)
}`,
    curl: `<span class="c"># Fingerprint anything.</span>
$ curl https://api.ucfp.dev/v1/fingerprint \\
    -H <span class="s">"content-type: application/octet-stream"</span> \\
    --data-binary @./model.safetensors

<span class="c"># {</span>
<span class="c">#   "id":   "ucfp1·b3k4q7n2…wpx9",</span>
<span class="c">#   "bits": 256,</span>
<span class="c">#   "modality": "weights"</span>
<span class="c"># }</span>`
  };

  let lang = $state<Lang>('ts');
</script>

<section class="use" id="use">
  <div>
    <div class="section-label">02 · Usage</div>
    <h2 class="h">One call. <span class="it">Any input.</span></h2>
    <p class="body">
      The SDK is a single function. Pass anything that can be serialized to bytes — a string, a
      Buffer, a stream, a file handle, an embedding vector — and get back a UCFP identifier.
    </p>
    <p class="body">
      IDs are stable across runtimes, platforms, and language bindings. The same input always
      returns the same fingerprint. Different inputs return different fingerprints with cryptographic
      confidence.
    </p>
  </div>
  <div>
    <div class="code-tabs">
      <button class:on={lang === 'ts'} onclick={() => (lang = 'ts')}>typescript</button>
      <button class:on={lang === 'py'} onclick={() => (lang = 'py')}>python</button>
      <button class:on={lang === 'go'} onclick={() => (lang = 'go')}>go</button>
      <button class:on={lang === 'curl'} onclick={() => (lang = 'curl')}>curl</button>
    </div>
    <div class="code-wrap">
      <pre class="code">{@html SNIPPETS[lang]}</pre>
    </div>
  </div>
</section>
