// Loading animation: the "searching" thinking orb (https://libraries.dev/orbs, js/vendor/thinking-orbs-engine.js),
// a dotted globe with a scan line, drawn on the #loading-animation canvas while the page shows it.
// The pages only switch the element's display on and off, so this watches that.
(function () {
    const SIZE = 64;

    let canvas = document.getElementById('loading-animation');
    let engine = window.ThinkingOrbsEngine;
    let preset = engine.searching;
    let dpr = Math.min(2, window.devicePixelRatio || 1);

    canvas.width = canvas.height = Math.round(SIZE * dpr);

    let ctx = canvas.getContext('2d');
    let reducedMotion = matchMedia('(prefers-reduced-motion: reduce)').matches;
    let frameRequest = 0;

    function draw() {
        // Light dots on the dark theme, dark dots on the light one.
        let dark = document.documentElement.dataset.theme != 'light';
        let time = reducedMotion ? 0.6 : performance.now() / 1000 * preset.speed;

        ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
        ctx.clearRect(0, 0, SIZE, SIZE);
        engine.paintFrame(ctx, preset.frame(SIZE, time, preset.opts), dark);
    }

    function loop() {
        draw();
        frameRequest = requestAnimationFrame(loop);
    }

    function update() {
        let shown = canvas.style.display != '' && canvas.style.display != 'none';

        cancelAnimationFrame(frameRequest);

        if (shown && !reducedMotion)
            loop();
        else if (shown)
            draw();
    }

    new MutationObserver(update).observe(canvas, { attributes: true, attributeFilter: ['style'] });
    update();
})();
