// @ts-check
/**
 * Type definitions for TaskStatus
 *
 * @typedef {Object} Task
 * @property {string} title
 * @property {string} video_id
 * @property {string} video_picture
 * @property {string} channel_name
 * @property {string} channel_id
 * @property {string} channel_picture
 * @property {string} output_directory
 *
 * @typedef {string|{[key:string]:any}} StatusState
 *
 * @typedef {Object} Status
 * @property {string} version
 * @property {StatusState} state
 * @property {string} last_output
 * @property {string} last_update
 * @property {string|null} video_fragments
 * @property {string|null} audio_fragments
 * @property {string|null} total_size
 * @property {string|null} video_quality
 * @property {string|null} output_file
 *
 * @typedef {Object} TaskStatus
 * @property {Task} task
 * @property {Status} status
 */

(() => {
  /**
   * Create a new Element
   *
   * @param {string} tagName
   * @param {Array<string | Node>} children
   * @param {Object} [props]
   * @returns {Node}
   */
  const el = (tagName, children, props) => {
    const el = document.createElement(tagName);
    if (props) {
      Object.keys(props).forEach((key) => {
        el.setAttribute(key, props[key]);
      });
    }
    for (let child of children) {
      if (typeof child === 'string') child = document.createTextNode(child);
      el.appendChild(child);
    }
    return el;
  };

  const stateSort = [
    'Idle',
    'Waiting',
    'Recording',
    'Muxing',
    'Finished',
    'AlreadyProcessed',
    'Interrupted',
  ];

  /** @param {StatusState} state */
  const stateString = (state) =>
    typeof state === 'string' ? state : Object.keys(state)[0];

  const cols = ['Thumbnail', 'Video', 'Status', 'Progress'];

  let lastStatus = '';
  const refresh = async () => {
    // Fetch the status
    const statusText = await fetch('/api/status')
      .then((res) => res.text())
      .catch(() => null);

    // Set up timeout to refetch
    setTimeout(refresh, 1000);

    // Skip update if the status is null or the same as last time
    if (statusText === null || statusText === lastStatus) return;
    lastStatus = statusText;

    /** @type {TaskStatus[]} */
    const statuses = JSON.parse(statusText);

    // Generate a table
    const table = el(
      'table',
      [
        el('thead', [
          el(
            'tr',
            cols.map((col) => el('th', [col]))
          ),
        ]),
        el(
          'tbody',
          statuses
            .sort(
              (a, b) =>
                stateSort.indexOf(stateString(a.status.state)) -
                stateSort.indexOf(stateString(b.status.state))
            )
            .map(({ task, status }) =>
              el('tr', [
                el('td', [
                  el('img', [], {
                    src: task.video_picture,
                    style: 'max-width:150px',
                  }),
                ]),
                el('td', [
                  el('a', [el('div', [task.title])], {
                    href: 'https://www.youtube.com/watch?v=' + task.video_id,
                  }),
                  el('a', [el('div', [task.channel_name])], {
                    href: 'https://www.youtube.com/channel/' + task.channel_id,
                    class: 'secondary',
                  }),
                ]),
                el('td', [stateString(status.state)]),
                el(
                  'td',
                  status.total_size === null
                    ? ['None']
                    : [
                        el('div', ['Video: ' + status.video_fragments]),
                        el('div', ['Audio: ' + status.audio_fragments]),
                        el('div', [`${status.total_size}`]),
                      ]
                ),
              ])
            )
        ),
      ],
      { role: 'grid' }
    );

    // Replace the table
    document.getElementById('status').replaceChildren(table);
  };

  refresh();
})();
