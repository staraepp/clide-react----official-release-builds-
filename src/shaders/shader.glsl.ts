/**
 * clide's ambient fluid field.
 *
 * Adapted from the marketing site's WebGL2 aurora, but intentionally kept to
 * one WebGL1 fullscreen pass for the app. The site's simplex/domain-warped
 * motion is preserved; its pointer flowmap, bloom, grain, and extra grid pass
 * are not. That keeps an idle dictation utility inexpensive and compatible
 * with the existing Reduced / Normal / High runtime controls.
 *
 * There is one field, at two intensities. At rest it renders in cool blue-grey
 * so the window has depth instead of being flat paper; as `u_presence` rises it
 * gathers into the site's oceanic blues. Idle is therefore atmosphere rather
 * than a second decorative object competing with the ribbon, and the product
 * rule that blue means voice still holds.
 */

export const VERTEX_SHADER = /* glsl */ `
attribute vec2 a_position;
void main() {
  gl_Position = vec4(a_position, 0.0, 1.0);
}
`;

export const FRAGMENT_SHADER = /* glsl */ `
#ifdef GL_FRAGMENT_PRECISION_HIGH
precision highp float;
#else
precision mediump float;
#endif

uniform vec2  u_resolution;
uniform float u_time;
/** 0 = reduced, 0.5 = normal, 1 = high. */
uniform float u_intensity;
/** Eased 0..1 while clide is capturing, transcribing, or inserting. */
uniform float u_presence;
/** Microphone level, 0..1. Only non-zero at High intensity while capturing. */
uniform float u_energy;

vec3 mod289(vec3 x) {
  return x - floor(x * (1.0 / 289.0)) * 289.0;
}

vec4 mod289(vec4 x) {
  return x - floor(x * (1.0 / 289.0)) * 289.0;
}

vec4 permute(vec4 x) {
  return mod289(((x * 34.0) + 1.0) * x);
}

vec4 taylorInvSqrt(vec4 r) {
  return 1.79284291400159 - 0.85373472095314 * r;
}

// 3D simplex noise from the same family used by clide's marketing-site
// aurora. Time is the third dimension, so motion stays fluid rather than
// looking like a 2D texture sliding across the window.
float snoise(vec3 v) {
  const vec2 C = vec2(1.0 / 6.0, 1.0 / 3.0);
  const vec4 D = vec4(0.0, 0.5, 1.0, 2.0);

  vec3 i = floor(v + dot(v, C.yyy));
  vec3 x0 = v - i + dot(i, C.xxx);
  vec3 g = step(x0.yzx, x0.xyz);
  vec3 l = 1.0 - g;
  vec3 i1 = min(g.xyz, l.zxy);
  vec3 i2 = max(g.xyz, l.zxy);
  vec3 x1 = x0 - i1 + C.xxx;
  vec3 x2 = x0 - i2 + C.yyy;
  vec3 x3 = x0 - D.yyy;

  i = mod289(i);
  vec4 p = permute(
    permute(
      permute(i.z + vec4(0.0, i1.z, i2.z, 1.0))
      + i.y + vec4(0.0, i1.y, i2.y, 1.0)
    ) + i.x + vec4(0.0, i1.x, i2.x, 1.0)
  );

  float n_ = 1.0 / 7.0;
  vec3 ns = n_ * D.wyz - D.xzx;
  vec4 j = p - 49.0 * floor(p * ns.z * ns.z);
  vec4 x_ = floor(j * ns.z);
  vec4 y_ = floor(j - 7.0 * x_);
  vec4 x = x_ * ns.x + ns.yyyy;
  vec4 y = y_ * ns.x + ns.yyyy;
  vec4 h = 1.0 - abs(x) - abs(y);
  vec4 b0 = vec4(x.xy, y.xy);
  vec4 b1 = vec4(x.zw, y.zw);
  vec4 s0 = floor(b0) * 2.0 + 1.0;
  vec4 s1 = floor(b1) * 2.0 + 1.0;
  vec4 sh = -step(h, vec4(0.0));
  vec4 a0 = b0.xzyw + s0.xzyw * sh.xxyy;
  vec4 a1 = b1.xzyw + s1.xzyw * sh.zzww;
  vec3 p0 = vec3(a0.xy, h.x);
  vec3 p1 = vec3(a0.zw, h.y);
  vec3 p2 = vec3(a1.xy, h.z);
  vec3 p3 = vec3(a1.zw, h.w);

  vec4 norm = taylorInvSqrt(vec4(
    dot(p0, p0),
    dot(p1, p1),
    dot(p2, p2),
    dot(p3, p3)
  ));
  p0 *= norm.x;
  p1 *= norm.y;
  p2 *= norm.z;
  p3 *= norm.w;

  vec4 m = max(
    0.6 - vec4(dot(x0, x0), dot(x1, x1), dot(x2, x2), dot(x3, x3)),
    0.0
  );
  m *= m;
  return 42.0 * dot(
    m * m,
    vec4(dot(p0, x0), dot(p1, x1), dot(p2, x2), dot(p3, x3))
  );
}

// Two stages of domain warping create the website shader's liquid folds with
// five noise evaluations and no textures or off-screen framebuffers.
float fluidNoise(vec2 uv, float time) {
  float n1 = snoise(vec3(uv * 0.62, time * 0.32));
  float n2 = snoise(vec3(uv * 0.62 + 5.2, time * 0.30 + 1.3));
  vec2 firstWarp = vec2(n1, n2) * 0.56;

  float n3 = snoise(vec3((uv + firstWarp) * 0.72 + 1.7, time * 0.27 + 3.1));
  float n4 = snoise(vec3((uv + firstWarp) * 0.72 + 9.2, time * 0.25 + 5.7));
  vec2 secondWarp = vec2(n3, n4) * 0.48;

  return snoise(vec3((uv + firstWarp + secondWarp) * 0.56, time * 0.22));
}

void main() {
  vec2 uv = gl_FragCoord.xy / u_resolution;
  float aspect = u_resolution.x / u_resolution.y;
  vec2 fieldUv = vec2(uv.x * aspect, uv.y);

  // Slow at Normal, a little more alive at High. Reduced is rendered once by
  // the React runtime with u_time fixed at zero.
  float time = u_time * mix(0.30, 0.62, u_intensity);
  fieldUv = fieldUv * 1.62 + vec2(-0.72, -0.28);
  fieldUv += vec2(time * 0.055, -time * 0.028);

  float primary = fluidNoise(fieldUv, time) * 0.5 + 0.5;
  float crossFlow = snoise(vec3(
    fieldUv * 0.78 + vec2(3.7, -2.1),
    time * 0.18 + 8.0
  )) * 0.5 + 0.5;

  // Thin contour bands make the field read as an aurora rather than generic
  // cloudy noise. Energy widens and brightens them while the user speaks.
  float contourPhase = primary * 2.25 + crossFlow * 0.48 + uv.y * 0.22;
  float contourDistance = abs(fract(contourPhase) - 0.5);
  float ribbon = 1.0 - smoothstep(
    0.10,
    0.32 + u_energy * 0.055,
    contourDistance
  );
  float broadField = smoothstep(0.18, 0.86, primary + crossFlow * 0.14);

  vec3 paper = vec3(0.965, 0.965, 0.961);
  vec3 pearl = vec3(0.992, 0.992, 0.99);
  vec3 neutralMist = vec3(0.925, 0.925, 0.92);
  vec3 neutralDeep = vec3(0.875, 0.875, 0.87);

  // Marketing-site ocean palette. It is mixed in only through presence.
  vec3 siteSoft = vec3(0.92, 0.92, 0.915);
  vec3 siteCyan = vec3(0.72, 0.72, 0.71);
  vec3 siteBlue = vec3(0.42, 0.42, 0.41);
  vec3 voiceDeep = vec3(0.25, 0.25, 0.25);

  float edgeDistance = length((uv - 0.5) * vec2(1.08, 0.92));
  float edgeFade = 1.0 - smoothstep(0.43, 0.82, edgeDistance);
  float presence = smoothstep(0.0, 1.0, u_presence);

  // The resting field fades back as voice takes over, so the two never stack
  // into mud. Weighted toward the open lower canvas but not confined to it —
  // the gutters between the bento cards should carry the field too.
  float lowerBias = 0.34 + 0.66 * smoothstep(1.05, -0.05, uv.y);
  float restingScale = 1.0 - presence * 0.62;
  float neutralWeight =
    (0.52 + 0.30 * u_intensity) * edgeFade * lowerBias * restingScale;

  // A second, faster current crossing the first. One drifting layer reads as a
  // gradient that happens to change; two moving against each other read as
  // something flowing.
  float current = snoise(vec3(
    fieldUv * 1.34 + vec2(-2.4, 4.8),
    time * 0.44 + 11.0
  )) * 0.5 + 0.5;
  float currentBand = 1.0 - smoothstep(0.0, 0.42, abs(current - 0.5));

  vec3 color = mix(paper, pearl, broadField * neutralWeight * 1.4);
  color = mix(
    color,
    neutralMist,
    ribbon * neutralWeight * (0.78 + 0.30 * u_intensity)
  );
  // The crossing current, so the motion has a direction you can follow.
  color = mix(
    color,
    neutralMist,
    currentBand * neutralWeight * 0.42
  );
  // Only the densest folds reach the deeper tone, which keeps the field
  // reading as depth rather than as a pattern laid over the page.
  color = mix(
    color,
    neutralDeep,
    pow(ribbon, 2.4) * neutralWeight * (0.62 + 0.28 * u_intensity)
  );

  float voiceWeight = presence
    * (0.22 + 0.22 * u_intensity + 0.13 * u_energy)
    * edgeFade;

  color = mix(color, siteSoft, broadField * voiceWeight);
  color = mix(color, siteCyan, ribbon * voiceWeight * 0.58);
  color = mix(
    color,
    siteBlue,
    ribbon * ribbon * voiceWeight * (0.30 + 0.18 * u_energy)
  );
  color = mix(
    color,
    voiceDeep,
    pow(ribbon, 4.0) * voiceWeight * u_intensity * 0.12
  );

  // The website's offset light source, toned down so white cards remain crisp.
  float lightDistance = length((uv - vec2(0.86, 0.34)) * vec2(aspect, 1.0));
  float halo = exp(-lightDistance * 2.8);
  color = mix(color, vec3(1.0), halo * (0.028 + presence * 0.035));

  gl_FragColor = vec4(color, 1.0);
}
`;
