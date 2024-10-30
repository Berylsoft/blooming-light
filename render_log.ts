import { createCanvas } from "jsr:@gfx/canvas@0.5.6";

// NOTE: require ffmpeg

const outputFile = "output.mov";
const outputAddtionalArgs: string[] = ["-c:v", "prores_videotoolbox"];
const inputFile = "filtered.jsonl";
// minimum spacing by px
const spacing = 5;
// move distance per second: width * this
const speedPPS = 0.1;
const fps = 60;
const width = 1920;
const height = 1080;

const canvas = createCanvas(width, height);
const ctx = canvas.getContext("2d");
ctx.font = `"24px Source Han Sans"`;
ctx.fillStyle = "#eee";

interface DanmakuItem {
    msg: string;
    width: number;
    x: number;
}

const pending: DanmakuItem[] = [];
const slots: DanmakuItem[][] = [];
let slotHeight = 114514;
let fontBoundingBoxAscent = 114514;

function assert(condition: boolean, message: string) {
    if (!condition) {
        console.trace(message);
        Deno.exit(-1);
    }
}

function assertNonNull<T>(value: T): NonNullable<T> {
    assert(value != null, "unexpected null value");
    return value!;
}

function pushMessage(msg: string) {
    const ctx = assertNonNull(canvas.getContext("2d"));
    const text = ctx.measureText(msg);
    pending.push({ msg: msg, width: text.width, x: 0 });
    if (slotHeight == 114514) {
        slotHeight = text.fontBoundingBoxAscent + text.fontBoundingBoxDescent;
    }
    if (fontBoundingBoxAscent == 114514) {
        fontBoundingBoxAscent = text.fontBoundingBoxAscent;
    }
}

function update(deltaTime: number) {
    const height = canvas.height;
    const width = canvas.width;

    ctx.clearRect(0, 0, width, height);

    const slotNum = Math.floor(height / slotHeight);

    for (let i = 0; i < slotNum; i++) {
        if (pending.length === 0) break;

        if (slots[i] == null) {
            slots[i] = [];
        }
        const slot = slots[i];

        const lastItem = slot.at(-1);
        if (lastItem) {
            const space = width - (lastItem.x + lastItem.width);
            if (space < spacing) continue;
        }

        const item = assertNonNull(pending.shift());
        item.x = width;
        slot.push(item);
    }

    for (const slotIdx in slots) {
        const y = slotHeight * (+slotIdx) + fontBoundingBoxAscent;
        const slot = slots[slotIdx];
        const needDelete: number[] = [];

        for (const i in slot) {
            const item = slot[i];

            if (item == null) {
                continue;
            }

            if (item.x < -item.width) {
                needDelete.push(+i);
            } else {
                ctx.fillText(item.msg, item.x, y);
                //ctx.strokeRect(
                //  item.x,
                //  y - fontBoundingBoxAscent,
                //  item.width,
                //  slotHeight,
                //);

                item.x -= speedPPS * width * deltaTime;
            }
        }

        needDelete.forEach((it) => {
            slot.splice(it, 1);
        });
    }
}

function sceneNotEmpty(): boolean {
    if (pending.length > 0) {
        return true;
    }
    for (const slot of slots) {
        if (slot.length > 0) {
            return true;
        }
    }
    return false;
}

interface LogEntry {
    msg: string;
    is_delete: boolean;
    ts: number;
}

const inputStr = Deno.readTextFileSync(inputFile);
const input: LogEntry[] = inputStr.trimEnd().split("\n").map((it) => {
    const entry = JSON.parse(it);
    return {
        ...entry,
        ts: Date.parse(entry["ts"]),
    };
});
input.sort((a, b) => a.ts - b.ts);

const startTs = input[0]?.ts || 0;
for (const entry of input) {
    entry.ts -= startTs;
}
const minTotalFrame = Math.ceil((input.at(-1)?.ts || 0) / 1000 * fps);

const ffmpegProcess = new Deno.Command("ffmpeg", {
    args: [
        "-y",
        "-f",
        "rawvideo",
        "-video_size",
        `${width}x${height}`,
        "-pixel_format",
        "rgba",
        "-framerate",
        fps.toString(10),
        "-i",
        "-",
        ...outputAddtionalArgs,
        outputFile,
    ],
    stdin: "piped",
}).spawn();
const ffmpegWriter = ffmpegProcess.stdin.getWriter();

let frameCount = 0;
const dt = 1 / fps;
const startTime = performance.now();
while (input.length > 0 || sceneNotEmpty()) {
    const ts = dt * frameCount * 1000;

    while (input[0]?.ts < ts) {
        pushMessage(assertNonNull(input.shift()).msg);
    }

    update(dt);
    const pixels = canvas.readPixels();
    //Deno.writeFileSync("test.bin", pixels, { append: true });
    await ffmpegWriter.write(pixels);

    const dur = performance.now() - startTime;
    if (frameCount % 500 === 0) {
        const frameTime = dur / frameCount;
        console.log(
            `%s%s/%d(min) %s%%(max) %sms/f(avg), estimated: %ss`,
            "[2K",
            frameCount,
            minTotalFrame,
            (frameCount / minTotalFrame * 100).toFixed(2),
            frameTime.toFixed(2),
            ((minTotalFrame - frameCount) * frameTime / 1000).toFixed(2),
        );
    }
    frameCount += 1;
}

ffmpegWriter.releaseLock();
await ffmpegProcess.stdin.close();
ffmpegProcess.ref();
