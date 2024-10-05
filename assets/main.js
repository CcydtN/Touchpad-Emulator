function startup() {
	const el = document.getElementById("canvas");
	el.addEventListener("touchstart", handleStart);
	el.addEventListener("touchend", handleEnd);
	el.addEventListener("touchcancel", handleCancel);
	el.addEventListener("touchmove", handleMove);
	log("Initialized.");
}

document.addEventListener("DOMContentLoaded", startup);

const ongoingTouches = [];

function handleStart(evt) {
	evt.preventDefault();
	const el = document.getElementById("canvas");
	const ctx = el.getContext("2d");
	const touches = getTouches(el, evt.changedTouches);

	for (touch of touches) {
		log(`touchstart, id: ${touch.identifier}`);
		ongoingTouches.push(touch);
		const color = colorForTouch(touch);
		log(`color of touch with id ${touch.identifier} = ${color}`);
		ctx.beginPath();
		ctx.arc(touch.x, touch.y, 4, 0, 2 * Math.PI, false); // a circle at the start
		ctx.fillStyle = color;
		ctx.fill();
	}

	send_post("touchstart", touches);
}

function handleMove(evt) {
	evt.preventDefault();
	const el = document.getElementById("canvas");
	const ctx = el.getContext("2d");
	const touches = getTouches(el, evt.changedTouches);

	for (touch of touches) {
		const color = colorForTouch(touch);
		const idx = ongoingTouchIndexById(touch.identifier);

		if (idx >= 0) {
			log(`continuing touch ${idx}`);
			ctx.beginPath();
			log(`ctx.moveTo( ${ongoingTouches[idx].x}, ${ongoingTouches[idx].y} );`);
			ctx.moveTo(ongoingTouches[idx].x, ongoingTouches[idx].y);
			log(`ctx.lineTo( ${touch.x}, ${touch.y} );`);
			ctx.lineTo(touch.x, touch.y);
			ctx.lineWidth = 4;
			ctx.strokeStyle = color;
			ctx.stroke();

			ongoingTouches.splice(idx, 1, touch); // swap in the new touch record
		} else {
			log("can't figure out which touch to continue");
		}
	}
	send_post("touchmove", touches);
}

function handleEnd(evt) {
	evt.preventDefault();
	const el = document.getElementById("canvas");
	const ctx = el.getContext("2d");
	const touches = getTouches(el, evt.changedTouches);
	log("touchend");

	for (touch of touches) {
		const color = colorForTouch(touch);
		const idx = ongoingTouchIndexById(touch.identifier);

		if (idx >= 0) {
			ctx.lineWidth = 4;
			ctx.fillStyle = color;
			ctx.beginPath();
			ctx.moveTo(ongoingTouches[idx].x, ongoingTouches[idx].y);
			ctx.lineTo(touch.x, touch.y);
			ctx.fillRect(touch.x - 4, touch.y - 4, 8, 8); // and a square at the end
			ongoingTouches.splice(idx, 1); // remove it; we're done
		} else {
			log("can't figure out which touch to end");
		}
	}

	send_post("touchend", touches);
}

function handleCancel(evt) {
	evt.preventDefault();
	log("touchcancel.");
	const el = document.getElementById("canvas");
	const touches = getTouches(el, evt.changedTouches);

	for (touch of touches) {
		const idx = ongoingTouchIndexById(touch.identifier);
		ongoingTouches.splice(idx, 1); // remove it; we're done
	}

	send_post("touchcancel", touches);
}

function colorForTouch(touch) {
	let r = touch.identifier % 16;
	let g = Math.floor(touch.identifier / 3) % 16;
	let b = Math.floor(touch.identifier / 7) % 16;
	r = r.toString(16); // make it a hex digit
	g = g.toString(16); // make it a hex digit
	b = b.toString(16); // make it a hex digit
	const color = `#${r}${g}${b}`;
	return color;
}

const clamp = (num, min, max) => Math.min(Math.max(num, min), max);

function getTouches(canvasDom, touchList) {
	const rect = canvasDom.getBoundingClientRect();
	const touches = [];
	for (let i = 0; i < touchList.length; i++) {
		const touch = {
			identifier: touchList[i].identifier,
			x: clamp(touchList[i].clientX - rect.left, 0, rect.width - 2),
			y: clamp(touchList[i].clientY - rect.top, 0, rect.height - 2),
		};
		touches.push(touch);
	}
	return touches;
}

function ongoingTouchIndexById(idToFind) {
	for (let i = 0; i < ongoingTouches.length; i++) {
		const id = ongoingTouches[i].identifier;

		if (id === idToFind) {
			return i;
		}
	}
	return -1; // not found
}

function log(msg) {
	const container = document.getElementById("log");
	container.textContent = `${msg} \n${container.textContent}`;
}

function send_post(end_point, touches) {
	const xhr = new XMLHttpRequest();
	xhr.open("POST", `${window.location.origin}/${end_point}`);
	xhr.setRequestHeader("Content-Type", "application/json");
	const body = JSON.stringify({ touches: touches });
	xhr.send(body);
}
