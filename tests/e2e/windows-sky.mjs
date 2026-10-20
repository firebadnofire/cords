// Reusable Windows UIA driver for the Computer Use @oai/sky runtime.
// Each call performs one action only, followed by observation. Inspect it before acting again.
import fs from 'node:fs/promises';
import path from 'node:path';

export async function createRunner(sky, executable, directory) {
    await fs.mkdir(directory, { recursive: true });
    const candidates = (await sky.list_windows()).filter(w => w.app === `process:${executable}`);
    if (candidates.length !== 1) throw new Error(`Expected one owned Cords window; found ${candidates.length}`);
    let window = await sky.get_window(candidates[0]);
    let state;
    let sequence = 0;
    async function observe() {
        state = await sky.get_window_state({ window, include_screenshot: true, include_text: true });
        window = state.window;
        const prefix = path.join(directory, String(++sequence).padStart(4, '0'));
        await fs.writeFile(prefix + '.json', JSON.stringify({ window, accessibility: state.accessibility }, null, 2));
        // Persist requested evidence; the runtime already displays the image for inspection.
        for (const [index, shot] of state.screenshots.entries()) {
            const match = /^data:image\/(png|jpeg);base64,(.*)$/s.exec(shot.url);
            if (!match) throw new Error('Unsupported screenshot encoding');
            await fs.writeFile(`${prefix}-${index}.${match[1] === 'jpeg' ? 'jpg' : 'png'}`, Buffer.from(match[2], 'base64'));
        }
        return state;
    }
    function index(identifier) {
        if (!state?.accessibility) throw new Error('Observe before acting');
        const matches = state.accessibility.tree.split('\n').filter(line => line.trimEnd().endsWith('.' + identifier));
        if (matches.length !== 1) throw new Error(`Expected one accessible identifier ${identifier}; found ${matches.length}`);
        return Number(matches[0].trim().match(/^\d+/)[0]);
    }
    async function clickName(name) {
        if (!state?.accessibility) throw new Error('Observe before acting');
        const matches = state.accessibility.tree.split('\n').filter(line => line.includes(` ${name} ID:`) && /\d+ button /.test(line));
        if (matches.length !== 1) throw new Error(`Expected one accessible button ${name}; found ${matches.length}`);
        const element_index = Number(matches[0].trim().match(/^\d+/)[0]);
        state = null;
        await sky.click({ window, element_index });
        return observe();
    }
    async function action(method, identifier, extra = {}) {
        const element_index = index(identifier);
        state = null;
        await sky[method]({ window, element_index, ...extra });
        return observe();
    }
    async function clickAt(x, y) {
        const shot = state?.screenshots?.[0];
        if (!shot || x < 0 || y < 0 || x >= shot.width || y >= shot.height) {
            throw new Error('Coordinate must come from the current bounded screenshot');
        }
        state = null;
        await sky.click({ window, screenshotId: shot.id, x, y });
        return observe();
    }
    async function keyboard(method, fields) {
        if (!state) throw new Error('Inspect focused control before keyboard input');
        state = null;
        await sky[method]({ window, ...fields });
        return observe();
    }
    return { observe, clickName, clickAt,
        typeText: text => keyboard('type_text', { text }),
        pressKey: key => keyboard('press_key', { key }),
        click: id => action('click', id), set: (id, value) => action('set_value', id, { value }),
        assertText(text) {
            if (!state?.accessibility?.tree.includes(text)) throw new Error('Expected visible UI text: ' + text);
        } };
}
