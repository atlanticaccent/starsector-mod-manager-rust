document.addEventListener('DOMContentLoaded', _ => {
  document.head.appendChild(document.createElement("style")).innerHTML = `
    [data-component*="dialog"] * {
      box-sizing: border-box;
      outline-color: var(--dlg-outline-c, hsl(218, 79.19%, 35%))
    }
    :where([data-component*="dialog"]) {
      --dlg-gap: 1em;
      background: var(--dlg-bg, #fff);
      border: var(--dlg-b, 0);
      border-radius: var(--dlg-bdrs, 0.25em);
      box-shadow: var(--dlg-bxsh, 0px 25px 50px -12px rgba(0, 0, 0, 0.25));
      font-family:var(--dlg-ff, ui-sansserif, system-ui, sans-serif);
      min-inline-size: var(--dlg-mis, auto);
      padding: var(--dlg-p, var(--dlg-gap));
      width: var(--dlg-w, fit-content);
    }
    :where([data-component="no-dialog"]:not([hidden])) {
      display: block;
      inset-block-start: var(--dlg-gap);
      inset-inline-start: 50%;
      position: fixed;
      transform: translateX(-50%);
    }
    :where([data-component*="dialog"] menu) {
      display: flex;
      gap: calc(var(--dlg-gap) / 2);
      justify-content: var(--dlg-menu-jc, flex-end);
      margin: 0;
      padding: 0;
    }
    :where([data-component*="dialog"] menu button) {
      background-color: var(--dlg-button-bgc);
      border: 0;
      border-radius: var(--dlg-bdrs, 0.25em);
      color: var(--dlg-button-c);
      font-size: var(--dlg-button-fz, 0.8em);
      padding: var(--dlg-button-p, 0.65em 1.5em);
    }
    :where([data-component*="dialog"] [data-ref="accept"]) {
      --dlg-button-bgc: var(--dlg-accept-bgc, hsl(218, 79.19%, 46.08%));
      --dlg-button-c: var(--dlg-accept-c, #fff);
    }
    :where([data-component*="dialog"] [data-ref="cancel"]) {
      --dlg-button-bgc: var(--dlg-cancel-bgc, transparent);
      --dlg-button-c: var(--dlg-cancel-c, inherit);
    }
    :where([data-component*="dialog"] [data-ref="fieldset"]) {
      border: 0;
      margin: unset;
      padding: unset;
    }
    :where([data-component*="dialog"] [data-ref="message"]) {
      font-size: var(--dlg-message-fz, 1.25em);
      margin-block-end: var(--dlg-gap);
    }
    :where([data-component*="dialog"] [data-ref="template"]:not(:empty)) {
      margin-block-end: var(--dlg-gap);
      width: 100%;
    }
    
    /* hack for Firefox */
    @-moz-document url-prefix() { 
      [data-component="no-dialog"]:not([hidden]) {
        inset-inline-start: 0;
        transform: none;
      }
    }
    
    /* added to body when browser do not support <dialog> */
    .dialog-open {
      background-color: rgba(0, 0, 0, .1);
      overflow: hidden;
    }
    
    /* FOR DEMO */
    [name="prompt"] {
      border: 1px solid silver;
      padding: .6em 1em;
      width: 100%;
    }
    
    .custom {
      --dlg-accept-bgc: hsl(159, 65%, 75%);
      --dlg-accept-c: #000;
      --dlg-bg: linear-gradient(to bottom right,#00F5A0,#00D9F5);
      --dlg-button-p: 0.75em 2em;
      --dlg-outline-c: #00D9F5;
    }
    .custom input {
      background-color: rgba(255, 255, 255, .5);
      border-radius: .25em;
      border: 0;
      display: block;
      margin-block: .5em 1em;
      padding: .75em 1em;
      width: 100%;
    }
    .custom label {
      display: block;
      font-size: small;
    }
    
    button[id] {
      background-color: rgb(239, 239, 239);
      border: 1px solid rgb(118, 118, 118);
      border-radius: .25em;
      font-size: .8rem;
      margin-inline-end: .25em;
      padding: 1em 2em;
    }
    
    button[id]:hover {
      background-color: rgb(250, 250, 250);
      border-color: rgb(0, 0, 0);
      color: rgb(0, 0, 0);
    }
  `;

  window.ipc.postMessage("pageLoaded:");
});

window.addEventListener("beforeunload", (_) => {
  console.log("sending page unload message");
  window.ipc.postMessage("pageUnload:");
});

// Adds an URL.getFromObjectURL( <blob:// URI> ) method
// returns the original object (<Blob> or <MediaSource>) the URI points to or null
(() => {
  // overrides URL methods to be able to retrieve the original blobs later on
  const old_create = URL.createObjectURL;
  const old_revoke = URL.revokeObjectURL;
  Object.defineProperty(URL, 'createObjectURL', {
    get: () => storeAndCreate
  });
  Object.defineProperty(URL, 'revokeObjectURL', {
    get: () => forgetAndRevoke
  });
  Object.defineProperty(URL, 'getFromObjectURL', {
    get: () => getBlob
  });
  Object.defineProperty(URL, 'getObjectURLDict', {
    get: () => getDict
  });
  Object.defineProperty(URL, 'clearURLDict', {
    get: () => clearDict
  });
  const dict = {};
  
  function storeAndCreate(blob) {
    const url = old_create(blob); // let it throw if it has to
    dict[url] = blob;
    console.log(blob)
    return url
  }
  
  function forgetAndRevoke(url) {
    console.log(`revoke ${url}`)
    old_revoke(url);
  }
  
  function getBlob(url) {
    return dict[url] || null;
  }
  
  function getDict() {
    return dict;
  }
  
  function clearDict() {
    dict = {};
  }
})();

/**
 * Dialog module.
 * @module dialog.js
 * @version 1.0.0
 * @summary 02-01-2022
 * @author Mads Stoumann
 * @description Custom versions of `alert`, `confirm` and `prompt`, using `<dialog>`
 */
class Dialog {
  constructor(settings = {}) {
    this.settings = Object.assign(
      {
        accept: 'OK',
        bodyClass: 'dialog-open',
        cancel: 'Cancel',
        dialogClass: '',
        message: '',
        soundAccept: '',
        soundOpen: '',
        template: ''
      },
      settings
    )
    this.init()
  }

  collectFormData(formData) {
    const object = {};
    formData.forEach((value, key) => {
      if (!Reflect.has(object, key)) {
        object[key] = value
        return
      }
      if (!Array.isArray(object[key])) {
        object[key] = [object[key]]
      }
      object[key].push(value)
    })
    return object
  }

  getFocusable() {
    return [...this.dialog.querySelectorAll('button,[href],select,textarea,input:not([type="hidden"]),[tabindex]:not([tabindex="-1"])')]
  }

  init() {
    this.dialogSupported = typeof HTMLDialogElement === 'function'
    this.dialog = document.createElement('dialog')
    this.dialog.role = 'dialog'
    this.dialog.dataset.component = this.dialogSupported ? 'dialog' : 'no-dialog';
    this.dialog.innerHTML = `
    <form method="dialog" data-ref="form">
      <fieldset data-ref="fieldset" role="document">
        <legend data-ref="message" id="${(Math.round(Date.now())).toString(36)}"></legend>
        <div data-ref="template"></div>
      </fieldset>
      <menu>
        <button${this.dialogSupported ? '' : ` type="button"`} data-ref="cancel" value="cancel"></button>
        <button${this.dialogSupported ? '' : ` type="button"`} data-ref="accept" value="default"></button>
      </menu>
      <audio data-ref="soundAccept"></audio>
      <audio data-ref="soundOpen"></audio>
    </form>`
    document.body.appendChild(this.dialog)

    this.elements = {}
    this.focusable = []
    this.dialog.querySelectorAll('[data-ref]').forEach(el => this.elements[el.dataset.ref] = el)
    this.dialog.setAttribute('aria-labelledby', this.elements.message.id)
    this.elements.cancel.addEventListener('click', () => { this.dialog.dispatchEvent(new Event('cancel')) })
    this.dialog.addEventListener('keydown', e => {
      if (e.key === 'Enter') {
        if (!this.dialogSupported) e.preventDefault()
        this.elements.accept.dispatchEvent(new Event('click'))
      }
      if (e.key === 'Escape') this.dialog.dispatchEvent(new Event('cancel'))
      if (e.key === 'Tab') {
        e.preventDefault()
        const len =  this.focusable.length - 1;
        let index = this.focusable.indexOf(e.target);
        index = e.shiftKey ? index - 1 : index + 1;
        if (index < 0) index = len;
        if (index > len) index = 0;
        this.focusable[index].focus();
      }
    })
    this.toggle()
  }

  open(settings = {}) {
    const dialog = Object.assign({}, this.settings, settings)
    this.dialog.className = dialog.dialogClass || ''
    this.elements.accept.innerText = dialog.accept
    this.elements.cancel.innerText = dialog.cancel
    this.elements.cancel.hidden = dialog.cancel === ''
    this.elements.message.innerText = dialog.message
    this.elements.soundAccept.src = dialog.soundAccept || ''
    this.elements.soundOpen.src = dialog.soundOpen || ''
    this.elements.target = dialog.target || ''
    this.elements.template.innerHTML = dialog.template || ''

    this.focusable = this.getFocusable()
    this.hasFormData = this.elements.fieldset.elements.length > 0

    if (dialog.soundOpen) {
      this.elements.soundOpen.play()
    }

    this.toggle(true)

    if (this.hasFormData) {
      this.focusable[0].focus()
      this.focusable[0].select()
    }
    else {
      this.elements.accept.focus()
    }
  }

  toggle(open = false) {
    if (this.dialogSupported && open) this.dialog.showModal()
    if (!this.dialogSupported) {
      document.body.classList.toggle(this.settings.bodyClass, open)
      this.dialog.hidden = !open
      if (this.elements.target && !open) {
        this.elements.target.focus()
      }
    }
  }

  waitForUser() {
    return new Promise(resolve => {
      this.dialog.addEventListener('cancel', () => { 
        this.toggle()
        resolve(false)
      }, { once: true })
      this.elements.accept.addEventListener('click', () => {
        let value = this.hasFormData ? this.collectFormData(new FormData(this.elements.form)) : true;
        if (this.elements.soundAccept.getAttribute('src').length > 0) this.elements.soundAccept.play()
        this.toggle()
        resolve(value)
      }, { once: true })
    })
  }

  confirm(message, config = { target: event.target }) {
    const settings = Object.assign({}, config, { message, template: '' })
    this.open(settings)
    return this.waitForUser()
  }
}