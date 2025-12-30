import { useEffect, useRef, useMemo } from "react";
import * as THREE from "three";
import { createNoise3D } from "simplex-noise";

interface VoiceOrbProps {
  isActive: boolean;
  volume?: number;
  size?: number;
}

export default function VoiceOrb({ isActive, volume = 0, size = 120 }: VoiceOrbProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const rendererRef = useRef<THREE.WebGLRenderer | null>(null);
  const sceneRef = useRef<THREE.Scene | null>(null);
  const groupRef = useRef<THREE.Group | null>(null);
  const cameraRef = useRef<THREE.PerspectiveCamera | null>(null);
  const ballRef = useRef<THREE.Mesh | null>(null);
  const originalPositionsRef = useRef<Float32Array | null>(null);
  const frameRef = useRef<number>(0);
  const isActiveRef = useRef(isActive);
  const volumeRef = useRef(volume);
  const noise = useMemo(() => createNoise3D(), []);

  // keep refs in sync
  useEffect(() => {
    isActiveRef.current = isActive;
    volumeRef.current = volume;
  }, [isActive, volume]);

  useEffect(() => {
    if (!containerRef.current) return;

    const container = containerRef.current;

    const scene = new THREE.Scene();
    const group = new THREE.Group();
    const camera = new THREE.PerspectiveCamera(20, 1, 1, 100);
    camera.position.set(0, 0, 100);
    camera.lookAt(scene.position);

    scene.add(camera);
    sceneRef.current = scene;
    groupRef.current = group;
    cameraRef.current = camera;

    const renderer = new THREE.WebGLRenderer({ alpha: true, antialias: true });
    renderer.setSize(size, size);
    renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
    container.appendChild(renderer.domElement);
    rendererRef.current = renderer;

    // orb geometry with vertex colors
    const geometry = new THREE.IcosahedronGeometry(10, 8);
    const material = new THREE.MeshBasicMaterial({
      vertexColors: true,
      wireframe: true,
    });

    // init vertex colors
    const colors = new Float32Array(geometry.attributes.position.count * 3);
    geometry.setAttribute("color", new THREE.BufferAttribute(colors, 3));

    const ball = new THREE.Mesh(geometry, material);
    ball.position.set(0, 0, 0);
    ballRef.current = ball;
    originalPositionsRef.current = geometry.attributes.position.array.slice() as Float32Array;

    group.add(ball);
    scene.add(group);

    // continuous render loop with morphing
    const render = () => {
      if (!groupRef.current || !rendererRef.current || !sceneRef.current || !cameraRef.current || !ballRef.current) {
        frameRef.current = requestAnimationFrame(render);
        return;
      }

      const time = performance.now();
      const mesh = ballRef.current;
      const geo = mesh.geometry as THREE.BufferGeometry;
      const posAttr = geo.getAttribute("position");
      const colorAttr = geo.getAttribute("color");
      const original = originalPositionsRef.current!;

      const active = isActiveRef.current;
      const vol = volumeRef.current;

      // rotate
      groupRef.current.rotation.y += 0.008;
      groupRef.current.rotation.x += 0.002;

      // morph and color each vertex
      const intensity = active ? 3 : 1.5;
      const effectiveVolume = active ? Math.max(0.15, vol) : 0.08;

      for (let i = 0; i < posAttr.count; i++) {
        const ox = original[i * 3];
        const oy = original[i * 3 + 1];
        const oz = original[i * 3 + 2];

        const vertex = new THREE.Vector3(ox, oy, oz);
        vertex.normalize();

        const rf = 0.00001;
        const noiseVal = noise(
          vertex.x + time * rf * 7,
          vertex.y + time * rf * 8,
          vertex.z + time * rf * 9
        );

        const offset = 10;
        const amp = 2.5;
        const distance = offset + effectiveVolume * 4 * intensity + noiseVal * amp * effectiveVolume * intensity;

        vertex.multiplyScalar(distance);
        posAttr.setXYZ(i, vertex.x, vertex.y, vertex.z);

        // multicolor based on position + time
        const hue = ((time * 0.02) + (i * 2) + (noiseVal * 60)) % 360;
        const color = new THREE.Color(`hsl(${hue}, 85%, 65%)`);
        colorAttr.setXYZ(i, color.r, color.g, color.b);
      }

      posAttr.needsUpdate = true;
      colorAttr.needsUpdate = true;
      geo.computeVertexNormals();

      rendererRef.current.render(sceneRef.current, cameraRef.current);
      frameRef.current = requestAnimationFrame(render);
    };
    render();

    return () => {
      cancelAnimationFrame(frameRef.current);
      renderer.dispose();
      geometry.dispose();
      material.dispose();
      if (container.contains(renderer.domElement)) {
        container.removeChild(renderer.domElement);
      }
    };
  }, [size, noise]);

  return (
    <div
      ref={containerRef}
      style={{ width: size, height: size }}
      className="flex items-center justify-center"
    />
  );
}
