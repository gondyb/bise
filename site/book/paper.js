/* the brand book on paper: the pencil filters, the kiss in Caveat, the cloud doodles,
   and the pen marks that draw themselves when their block comes in. goes with paper.css. */
(() => {
  // the pencil (grain + a little displacement) and the wobble of a hand-drawn frame
  document.body.insertAdjacentHTML("afterbegin", `<svg width="0" height="0" style="position:absolute" aria-hidden="true"><defs>
<filter id="pencil" x="-10%" y="-10%" width="120%" height="120%"><feTurbulence type="fractalNoise" baseFrequency=".9" numOctaves="2" seed="4" result="n"/><feDisplacementMap in="SourceGraphic" in2="n" scale="2.2" result="d"/><feTurbulence type="fractalNoise" baseFrequency="1.8" seed="9" result="g"/><feColorMatrix in="g" type="matrix" values="0 0 0 0 0  0 0 0 0 0  0 0 0 0 0  0 0 0 -1.1 1.3" result="m"/><feComposite in="d" in2="m" operator="in"/></filter>
<filter id="wobble" x="-5%" y="-5%" width="110%" height="110%"><feTurbulence type="fractalNoise" baseFrequency=".03" numOctaves="2" seed="7"/><feDisplacementMap in="SourceGraphic" scale="4"/></filter>
<pattern id="hatchB" width="5" height="5" patternUnits="userSpaceOnUse" patternTransform="rotate(-35)"><line x1="0" y1="0" x2="0" y2="5" stroke="#8a8174" stroke-width="1.5" opacity=".7"/></pattern>
</defs></svg>`);

  // the north wind: a colored-pencil cloud, blowing (its gusts start at its right edge), or asleep
  const cloud = (asleep) => `<svg class="doodle cloud" viewBox="0 0 260 120" aria-hidden="true"><g filter="url(#pencil)" fill="none" stroke-linecap="round" stroke-linejoin="round">
<path data-ink="graphite" d="M40 92 C16 92,10 66,32 60 C26 36,56 26,70 42 C78 18,118 16,126 40 C142 26,170 36,164 58 C188 60,188 92,164 94 Z" fill="url(#hatchB)" stroke="#6b645a" stroke-width="2.8"/>
<g data-ink="face" stroke="#1d1a17" stroke-width="2"><path d="M70 66 q6 5 12 0"/><path d="M104 66 q6 5 12 0"/></g>
<circle cx="66" cy="78" r="6" fill="#ef9aae" opacity=".5"/><circle cx="122" cy="78" r="6" fill="#ef9aae" opacity=".5"/>
${asleep ? '<path data-ink="graphite" d="M150 46 l8 0 l-8 8 l8 0" stroke="#6b645a" stroke-width="1.8"/>'
         : '<ellipse data-ink="face" cx="95" cy="80" rx="4" ry="4.6" stroke="#1d1a17" stroke-width="1.8"/><g class="gust" data-ink="gust" stroke="#8a8174" stroke-width="2"><path d="M182 74 C196 68,208 80,226 72 S246 66,254 70"/><path d="M180 86 C196 90,210 98,232 92"/><path d="M174 60 C188 52,200 56,216 48"/></g>'}
</g></svg>`;
  document.querySelectorAll("[data-doodle]").forEach((el) => { el.innerHTML = cloud(el.dataset.doodle === "asleep"); });

  // the kiss: split ':*' where it's drawn with the pen, so the * sits level with the colon (a mouth)
  for (const el of document.querySelectorAll(".hero .big .kiss, nav.side .logo .acc, .top b span, header.top .logo .k, .kiss-c"))
    if (el.textContent.trim() === ":*") { el.classList.add("kiss-c"); el.innerHTML = '<span class="kc">:</span><span class="ks">*</span>'; }

  // pen marks draw themselves once their block is on screen
  const io = new IntersectionObserver((es) => es.forEach((e) => { if (e.isIntersecting) { e.target.classList.add("in"); io.unobserve(e.target); } }), { threshold: .2 });
  document.querySelectorAll(".hero, section, .marks > div, .plate").forEach((el) => io.observe(el));
})();
