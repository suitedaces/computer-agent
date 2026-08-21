export declare function takeScreenshot(): Promise<string>;
export declare function mouseMove(x: number, y: number): Promise<string>;
export declare function click(x: number, y: number, button?: "left" | "right" | "middle", double?: boolean): Promise<string>;
export declare function drag(from: [number, number], to: [number, number]): Promise<string>;
export declare function typeText(text: string): Promise<string>;
export declare function pressKey(combo: string): Promise<string>;
export declare function scroll(x: number, y: number, direction: string, amount?: number): Promise<string>;
