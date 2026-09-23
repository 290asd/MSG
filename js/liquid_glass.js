// Test: WebGL liquid glass for the "Refract" effects style.
// The glass shader is from liquid-glass-js by Armagan Amcalar (MIT, github.com/dashersw/liquid-glass-js).
// Instead of an html2canvas snapshot of the page, the texture is built from what is really behind
// the cards (the blurred ambient layer and the current image or video), and one full-window canvas
// between the image and the cards draws every glass card.
// Glass in the top layer (settings window, menus, toasts) keeps the CSS glass.
/*
MIT License

Copyright (c) 2025 Armagan Amcalar

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
*/
(function () {
    const SCALE = 0.5; // texture resolution; the shader blurs it anyway

    const vsSource = `
    attribute vec2 a_position;
    attribute vec2 a_texcoord;
    varying vec2 v_texcoord;
    void main() {
      gl_Position = vec4(a_position, 0, 1);
      v_texcoord = a_texcoord;
    }`;

    // From container.js, with the page scroll removed (the texture is the window) and the tint colors as uniforms.
    const fsSource = `
    precision mediump float;
    uniform sampler2D u_image;
    uniform vec2 u_resolution;
    uniform vec2 u_textureSize;
    uniform float u_blurRadius;
    uniform float u_borderRadius;
    uniform vec2 u_containerPosition;
    uniform float u_edgeIntensity;
    uniform float u_rimIntensity;
    uniform float u_edgeDistance;
    uniform float u_rimDistance;
    uniform float u_cornerBoost;
    uniform float u_rippleEffect;
    uniform float u_tintOpacity;
    uniform vec3 u_topTint;
    uniform vec3 u_bottomTint;
    uniform float u_opacity;
    varying vec2 v_texcoord;

    float roundedRectDistance(vec2 coord, vec2 size, float radius) {
      vec2 center = size * 0.5;
      vec2 pixelCoord = coord * size;
      vec2 toCorner = abs(pixelCoord - center) - (center - radius);
      float outsideCorner = length(max(toCorner, 0.0));
      float insideCorner = min(max(toCorner.x, toCorner.y), 0.0);
      return (outsideCorner + insideCorner - radius);
    }

    bool isPill(vec2 size, float radius) {
      return abs(radius - size.y * 0.5) < 2.0 && size.x > size.y + 4.0;
    }

    float pillDistance(vec2 coord, vec2 size, float radius) {
      vec2 center = size * 0.5;
      vec2 pixelCoord = coord * size;
      vec2 capsuleStart = vec2(radius, center.y);
      vec2 capsuleEnd = vec2(size.x - radius, center.y);
      vec2 capsuleAxis = capsuleEnd - capsuleStart;
      if (length(capsuleAxis) > 0.0) {
        float t = clamp(dot(pixelCoord - capsuleStart, capsuleAxis) / dot(capsuleAxis, capsuleAxis), 0.0, 1.0);
        return length(pixelCoord - (capsuleStart + t * capsuleAxis)) - radius;
      }
      return length(pixelCoord - center) - radius;
    }

    void main() {
      vec2 coord = v_texcoord;
      vec2 pagePixel = u_containerPosition + (coord - 0.5) * u_resolution;
      vec2 textureCoord = pagePixel / u_textureSize;

      float distFromEdgeShape;
      vec2 shapeNormal;
      bool pill = isPill(u_resolution, u_borderRadius);
      if (pill) {
        distFromEdgeShape = -pillDistance(coord, u_resolution, u_borderRadius);
        vec2 pixelCoord = coord * u_resolution;
        vec2 capsuleStart = vec2(u_borderRadius, 0.5 * u_resolution.y);
        vec2 capsuleEnd = vec2(u_resolution.x - u_borderRadius, 0.5 * u_resolution.y);
        vec2 capsuleAxis = capsuleEnd - capsuleStart;
        float t = clamp(dot(pixelCoord - capsuleStart, capsuleAxis) / dot(capsuleAxis, capsuleAxis), 0.0, 1.0);
        vec2 normalDir = pixelCoord - (capsuleStart + t * capsuleAxis);
        shapeNormal = length(normalDir) > 0.0 ? normalize(normalDir) : vec2(0.0, 1.0);
      } else {
        distFromEdgeShape = -roundedRectDistance(coord, u_resolution, u_borderRadius);
        shapeNormal = normalize(coord - vec2(0.5));
      }
      distFromEdgeShape = max(distFromEdgeShape, 0.0);

      float minDim = min(u_resolution.x, u_resolution.y);
      float distFromEdge = distFromEdgeShape / minDim;
      float normalizedDistance = distFromEdge * minDim;
      float edgeIntensity = exp(-normalizedDistance * u_edgeDistance);
      float rimIntensity = exp(-normalizedDistance * u_rimDistance);
      float totalIntensity = edgeIntensity * u_edgeIntensity + rimIntensity * u_rimIntensity;
      vec2 baseRefraction = shapeNormal * totalIntensity;

      float cornerProximityX = min(coord.x, 1.0 - coord.x);
      float cornerProximityY = min(coord.y, 1.0 - coord.y);
      float cornerNormalized = max(cornerProximityX, cornerProximityY) * minDim;
      vec2 cornerRefraction = shapeNormal * exp(-cornerNormalized * 0.3) * u_cornerBoost;

      vec2 perpendicular = vec2(-shapeNormal.y, shapeNormal.x);
      vec2 textureRefraction = perpendicular * sin(distFromEdge * 25.0) * u_rippleEffect * rimIntensity;

      textureCoord += baseRefraction + cornerRefraction + textureRefraction;

      vec4 color = vec4(0.0);
      vec2 texelSize = 1.0 / u_textureSize;
      float sigma = u_blurRadius / 2.0;
      vec2 blurStep = texelSize * sigma;
      float totalWeight = 0.0;
      for (float i = -6.0; i <= 6.0; i += 1.0) {
        for (float j = -6.0; j <= 6.0; j += 1.0) {
          float distance = length(vec2(i, j));
          if (distance > 6.0) continue;
          float weight = exp(-(distance * distance) / (2.0 * sigma * sigma));
          color += texture2D(u_image, textureCoord + vec2(i, j) * blurStep) * weight;
          totalWeight += weight;
        }
      }
      color /= totalWeight;

      vec3 gradientTint = mix(u_topTint, u_bottomTint, coord.y);
      color = vec4(mix(color.rgb, gradientTint, u_tintOpacity), 1.0);

      float maskDistance = pill ? pillDistance(coord, u_resolution, u_borderRadius) : roundedRectDistance(coord, u_resolution, u_borderRadius);
      float mask = 1.0 - smoothstep(-1.0, 1.0, maskDistance);
      mask *= u_opacity;
      gl_FragColor = vec4(color.rgb * mask, mask);
    }`;

    let canvas, gl, loc, texture, on = false;
    const backdrop = document.createElement('canvas');
    const bctx = backdrop.getContext('2d');
    const ambientImage = new Image();
    let lastBackdrop = '', lastRects = '';
    // A cached image can be "complete" before its size is known, so a load always draws the backdrop again.
    ambientImage.onload = () => lastBackdrop = '';
    let options = { blur: 5, strength: 1, ripple: 1, corner: 1, tint: 0.35 };

    function setup() {
        canvas = document.createElement('canvas');
        canvas.id = 'liquid-glass-canvas';
        document.body.insertBefore(canvas, document.getElementById('content') || null);
        gl = canvas.getContext('webgl', { premultipliedAlpha: true });

        const program = gl.createProgram();
        for (const [type, source] of [[gl.VERTEX_SHADER, vsSource], [gl.FRAGMENT_SHADER, fsSource]]) {
            const shader = gl.createShader(type);
            gl.shaderSource(shader, source);
            gl.compileShader(shader);
            if (!gl.getShaderParameter(shader, gl.COMPILE_STATUS))
                console.error('Liquid glass shader:', gl.getShaderInfoLog(shader));
            gl.attachShader(program, shader);
        }
        gl.linkProgram(program);
        gl.useProgram(program);

        loc = name => gl.getUniformLocation(program, name);
        for (const [name, data] of [['a_position', [-1, -1, 1, -1, -1, 1, -1, 1, 1, -1, 1, 1]], ['a_texcoord', [0, 1, 1, 1, 0, 0, 0, 0, 1, 1, 1, 0]]]) {
            gl.bindBuffer(gl.ARRAY_BUFFER, gl.createBuffer());
            gl.bufferData(gl.ARRAY_BUFFER, new Float32Array(data), gl.STATIC_DRAW);
            const a = gl.getAttribLocation(program, name);
            gl.enableVertexAttribArray(a);
            gl.vertexAttribPointer(a, 2, gl.FLOAT, false, 0, 0);
        }

        gl.uniform1f(loc('u_edgeDistance'), 0.15);
        gl.uniform1f(loc('u_rimDistance'), 0.8);

        texture = gl.createTexture();
        gl.bindTexture(gl.TEXTURE_2D, texture);
        gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR);
        gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
        gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
        gl.uniform1i(loc('u_image'), 0);
        gl.enable(gl.BLEND);
        gl.blendFunc(gl.ONE, gl.ONE_MINUS_SRC_ALPHA);
    }

    // What is behind the cards, drawn the way the CSS draws it: theme color, blurred ambient layer, current slide.
    function drawBackdrop(w, h) {
        const root = getComputedStyle(document.documentElement);
        const ambient = document.getElementById('ambient');
        const ambientStyle = getComputedStyle(ambient);
        const url = (ambient.style.backgroundImage.match(/url\("?(.*?)"?\)$/) || [])[1];
        if (url && ambientImage.src != url)
            ambientImage.src = url;

        backdrop.width = Math.ceil(w * SCALE);
        backdrop.height = Math.ceil(h * SCALE);
        bctx.setTransform(SCALE, 0, 0, SCALE, 0, 0);
        bctx.fillStyle = root.getPropertyValue('--bg') || '#000';
        bctx.fillRect(0, 0, w, h);

        if (url && ambientImage.complete && ambientImage.naturalWidth) {
            // #ambient: inset -10%, background-size cover.
            const aw = w * 1.2, ah = h * 1.2;
            const s = Math.max(aw / ambientImage.naturalWidth, ah / ambientImage.naturalHeight);
            const dw = ambientImage.naturalWidth * s, dh = ambientImage.naturalHeight * s;
            bctx.filter = ambientStyle.filter;
            bctx.globalAlpha = parseFloat(ambientStyle.opacity);
            bctx.drawImage(ambientImage, (w - dw) / 2, (h - dh) / 2, dw, dh);
            bctx.filter = 'none';
            bctx.globalAlpha = 1;
        }

        for (const el of [document.getElementById('current-image'), document.getElementById('current-video')]) {
            if (!el || el.style.display == 'none' || getComputedStyle(el).display == 'none')
                continue;
            const r = el.getBoundingClientRect();
            if (r.width && (el.naturalWidth || el.readyState >= 2))
                bctx.drawImage(el, r.left, r.top, r.width, r.height);
        }
    }

    function glassElements() {
        return [...document.querySelectorAll('.glass')].filter(el => !el.closest('dialog, [popover]'));
    }

    function frame() {
        if (!on)
            return;
        requestAnimationFrame(frame);

        const w = innerWidth, h = innerHeight;
        const img = document.getElementById('current-image'), video = document.getElementById('current-video');
        const backdropKey = [w, h, img?.src, img?.complete, video?.style.display, video?.currentTime, document.getElementById('ambient').style.backgroundImage, document.documentElement.dataset.theme].join('|');

        const els = glassElements();
        // With the element's opacity, so a card that is faded out (like the loading tip) has no glass either.
        const rects = els.map(el => {
            const r = el.getBoundingClientRect(), style = getComputedStyle(el);
            const opacity = style.visibility == 'hidden' ? 0 : +style.opacity;
            return [r.left, r.top, r.width, r.height].map(Math.round).concat(opacity);
        });
        const rectsKey = JSON.stringify(rects);

        if (backdropKey == lastBackdrop && rectsKey == lastRects)
            return;

        if (backdropKey != lastBackdrop) {
            drawBackdrop(w, h);
            gl.bindTexture(gl.TEXTURE_2D, texture);
            gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, gl.RGBA, gl.UNSIGNED_BYTE, backdrop);
            lastBackdrop = backdropKey;
        }
        lastRects = rectsKey;

        if (canvas.width != w || canvas.height != h) {
            canvas.width = w;
            canvas.height = h;
        }
        gl.viewport(0, 0, w, h);
        gl.clearColor(0, 0, 0, 0);
        gl.clear(gl.COLOR_BUFFER_BIT);
        gl.uniform2f(loc('u_textureSize'), w, h);

        const dark = document.documentElement.dataset.theme != 'light';
        gl.uniform3f(loc('u_topTint'), ...(dark ? [0.16, 0.17, 0.21] : [1, 1, 1]));
        gl.uniform3f(loc('u_bottomTint'), ...(dark ? [0.08, 0.09, 0.11] : [0.85, 0.86, 0.9]));
        gl.uniform1f(loc('u_tintOpacity'), options.tint);
        // The library's defaults times the settings (Settings → Appearance).
        gl.uniform1f(loc('u_blurRadius'), Math.max(options.blur, 0.1));
        gl.uniform1f(loc('u_edgeIntensity'), 0.01 * options.strength);
        gl.uniform1f(loc('u_rimIntensity'), 0.05 * options.strength);
        gl.uniform1f(loc('u_cornerBoost'), 0.02 * options.corner);
        gl.uniform1f(loc('u_rippleEffect'), 0.1 * options.ripple);

        els.forEach((el, i) => {
            const [x, y, rw, rh, opacity] = rects[i];
            if (rw < 2 || rh < 2 || y > h || y + rh < 0 || opacity == 0)
                return;
            const radius = Math.min(parseFloat(getComputedStyle(el).borderTopLeftRadius) || 0, rw / 2, rh / 2);
            gl.viewport(x, h - y - rh, rw, rh);
            gl.uniform2f(loc('u_resolution'), rw, rh);
            gl.uniform2f(loc('u_containerPosition'), x + rw / 2, y + rh / 2);
            gl.uniform1f(loc('u_borderRadius'), radius);
            gl.uniform1f(loc('u_opacity'), opacity);
            gl.drawArrays(gl.TRIANGLES, 0, 6);
        });
    }

    function update(newOptions) {
        if (newOptions) {
            options = newOptions;
            lastRects = ''; // draw again with the new options
        }
        const wasOn = on;
        on = document.body.classList.contains('refract-style');
        if (on && !canvas)
            setup();
        if (canvas)
            canvas.style.display = on ? '' : 'none';
        for (const el of glassElements())
            el.classList.toggle('lg-drawn', on);
        if (on && !wasOn) {
            lastBackdrop = lastRects = '';
            requestAnimationFrame(frame);
        }
    }

    // Glass cards that show up later get the class too.
    // At most once a frame, however many elements changed.
    let marking = false;
    new MutationObserver(() => {
        if (!on || marking)
            return;
        marking = true;
        requestAnimationFrame(() => {
            marking = false;
            for (const el of glassElements())
                el.classList.add('lg-drawn');
        });
    }).observe(document.documentElement, { childList: true, subtree: true });

    window.LiquidGlass = { update };
})();
