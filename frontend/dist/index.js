/**
 * @type {HTMLCanvasElement}
 */
const canvas = document.querySelector("#canvas");

let speedPPS = 15_000;
const pending = [];
const slots = [];
let slotHeight = 114514;
let fontBoundingBoxAscent = 114514;

/**
 * @param {MessageEvent} ev
 */
function onMessage(ev) {
  const msg = ev.data;

  const ctx = canvas.getContext("2d");
  const text = ctx.measureText(msg);
  pending.push({ msg: msg, width: text.width });
  slotHeight = text.fontBoundingBoxAscent + text.fontBoundingBoxDescent;
  fontBoundingBoxAscent = text.fontBoundingBoxAscent;
}

let lastTime = performance.now();
function update() {
  const now = performance.now();
  const deltaTime = (now - lastTime) / 1000;
  lastTime = now;

  const rect = canvas.getBoundingClientRect();
  if (canvas.height != rect.height) canvas.height = rect.height;
  if (canvas.width != rect.width) canvas.width = rect.width;

  const ctx = canvas.getContext("2d");

  const style = window.getComputedStyle(canvas);
  ctx.font = style.font;
  ctx.fillStyle = style.color;

  let spacing = Number.parseFloat(style.getPropertyValue("--spacing"));
  if (!Number.isFinite(spacing)) {
    spacing = 0;
  }

  let newPPS = Number.parseFloat(style.getPropertyValue("--pps"));
  if (Number.isFinite(newPPS) && newPPS > 0) {
    speedPPS = newPPS;
  }

  const height = canvas.height;
  const width = canvas.width;

  ctx.clearRect(0, 0, width, height);

  let slotNum = Math.floor(height / slotHeight);

  for (let i = 0; i < slotNum; i++) {
    if (pending.length === 0) break;

    if (slots[i] == null) {
      slots[i] = [];
    }
    const slot = slots[i];

    if (slot.length !== 0) {
      const lastItem = slot.at(-1);
      const space = width - (lastItem.x + lastItem.width);
      //ctx.fillStyle = "green";
      //ctx.fillRect(lastItem.x + lastItem.width, i * slotHeight, 2, slotHeight);
      //ctx.fillRect(0, i * slotHeight, space, slotHeight);
      //ctx.fillStyle = "red";
      //ctx.fillRect(
      //  lastItem.x + lastItem.width,
      //  i * slotHeight,
      //  spacing + pending[0].width,
      //  slotHeight,
      //);
      //ctx.fillStyle = "yellow";
      //ctx.fillRect(
      //  lastItem.x + lastItem.width,
      //  i * slotHeight,
      //  spacing,
      //  slotHeight,
      //);
      //ctx.fillStyle = style.color;
      if (space < spacing) continue;
    }

    const item = pending.shift();
    item.x = width;
    slot.push(item);
  }
  speedPPS *= pending.length > 0 ? 1 + pending.length / 20 : 1;

  for (const slotIdx in slots) {
    const y = slotHeight * slotIdx + fontBoundingBoxAscent;
    const slot = slots[slotIdx];
    const needDelete = [];

    for (const i in slot) {
      const item = slot[i];

      if (item == null) {
        continue;
      }

      if (item.x < -item.width) {
        needDelete.push(i);
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
  window.requestAnimationFrame(update);
}
update();

let client = null;
let reconnectTimeout = null;

function reconnect() {
  if (reconnectTimeout == null)
    reconnectTimeout = setTimeout(() => {
      client = connect();

      reconnectTimeout = null;
    }, 1e3);
}

function connect() {
  const ws = new WebSocket(`ws://${window.location.host}/ws`);

  ws.onopen = () => {
    console.log("client connected");
  };
  ws.onmessage = onMessage;
  ws.onerror = (err) => {
    console.error(err);
    console.error("reconnecting");
    reconnect();
  };
  ws.onclose = () => {
    console.error("closed reconnecting");
    reconnect();
  };

  return ws;
}

client = connect();
