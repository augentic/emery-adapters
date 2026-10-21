<!-- Generated from design-system/ — do not edit manually. -->
<script setup lang="ts">
import { s } from "../ui-strings";
import { assets } from "../assets";
</script>

<template>
  <main class="vx-screen" style="display: flex; flex-direction: column; min-height: 100vh; align-items: flex-start; background: var(--vectis-color-color-0a0a0a)">
    <div style="display: flex; flex-direction: column; align-items: flex-start; align-self: stretch">
      <div style="display: flex; flex-direction: column; align-items: flex-start; align-self: stretch; min-height: 800px">
        <div style="display: grid; grid-auto-flow: column; grid-template-columns: minmax(0, 1fr) minmax(0, 1fr); align-items: stretch; align-self: stretch; flex: 1 1 0%; min-height: 0">
          <div style="display: flex; flex-direction: row; justify-content: center; align-items: center; background: var(--vectis-color-color-18181b); min-width: 0">
            <img alt="FastAPI" :src="assets['img-e3c1bb4ea8-fastapi-logo-light']" style="overflow: hidden; width: 341.38px; flex-shrink: 0; height: 64px; max-width: 100%" />
          </div>
          <div style="display: flex; flex-direction: column; align-items: stretch; row-gap: var(--vectis-spacing-space-16); column-gap: var(--vectis-spacing-space-16); padding: var(--vectis-spacing-space-40); min-width: 0">
            <div style="display: flex; flex-direction: row; justify-content: flex-end; align-items: stretch; align-self: stretch">
              <div style="display: flex; flex-direction: row; justify-content: center; align-items: center">
                <!-- TODO(event): unfilled hole, raises nothing -->
                <button class="vectis-type-apple-system-14-medium-lh20" type="button" style="background: var(--vectis-color-color-ffffff0b); border-radius: var(--vectis-corner-radius-radius-8); border: var(--vectis-border-border-1) solid var(--vectis-color-color-ffffff26); box-shadow: var(--vectis-box-shadow-shadow-677a96d2); white-space: nowrap; display: flex; flex-direction: row; align-items: center; justify-content: center; text-align: center; color: var(--vectis-color-color-fafafa); column-gap: 8px; flex-shrink: 0; row-gap: 8px" data-vx-hole="event">
                </button>
              </div>
            </div>
            <div style="display: flex; flex-direction: row; justify-content: center; align-items: center; align-self: stretch; flex: 1 1 0%; min-height: 0">
              <div style="display: flex; flex-direction: column; align-items: flex-start; max-width: 320px">
                <div style="display: flex; flex-direction: column; align-items: stretch; row-gap: var(--vectis-spacing-space-24); column-gap: var(--vectis-spacing-space-24); align-self: stretch">
                  <div style="display: flex; flex-direction: column; align-items: center; row-gap: var(--vectis-spacing-space-8); column-gap: var(--vectis-spacing-space-8); align-self: stretch">
                    <span class="vectis-type-apple-system-24-bold-lh32" style="color: var(--vectis-color-color-fafafa); text-align: center">{{ s.LOGIN_TO_YOUR_ACCOUNT }}</span>
                  </div>
                  <div style="display: flex; flex-direction: column; align-items: flex-start; row-gap: var(--vectis-spacing-space-16); align-self: stretch">
                    <div style="display: flex; flex-direction: column; align-items: flex-start; row-gap: var(--vectis-spacing-space-8); align-self: stretch">
                      <div style="display: flex; flex-direction: row; align-items: center; column-gap: var(--vectis-spacing-space-8); row-gap: var(--vectis-spacing-space-8); align-self: stretch">
                        <span class="vectis-type-apple-system-14-medium-lh14" style="color: var(--vectis-color-color-fafafa)">{{ s.EMAIL }}</span>
                      </div>
                      <div style="display: block; padding-top: var(--vectis-spacing-space-4); padding-right: var(--vectis-spacing-space-12); padding-bottom: var(--vectis-spacing-space-4); padding-left: var(--vectis-spacing-space-12); background: var(--vectis-color-color-ffffff0b); border-radius: var(--vectis-corner-radius-radius-8); border: var(--vectis-border-border-1) solid var(--vectis-color-color-ffffff26); box-shadow: var(--vectis-box-shadow-shadow-677a96d2); overflow: hidden; width: 320px; height: 36px; flex-shrink: 0">
                        <div aria-hidden="true" style="display: block"></div>
                      </div>
                    </div>
                    <div style="display: flex; flex-direction: column; align-items: flex-start; row-gap: var(--vectis-spacing-space-8); align-self: stretch">
                      <div style="display: flex; flex-direction: row; align-items: center; align-self: stretch">
                        <div style="display: flex; flex-direction: row; align-items: center; column-gap: var(--vectis-spacing-space-8); row-gap: var(--vectis-spacing-space-8)">
                          <span class="vectis-type-apple-system-14-medium-lh14" style="color: var(--vectis-color-color-fafafa)">{{ s.PASSWORD }}</span>
                        </div>
                        <div style="display: flex; flex-direction: column; align-items: stretch; padding-left: var(--vectis-spacing-space-108p86)">
                          <span class="vectis-type-apple-system-14-regular-lh20" style="color: var(--vectis-color-color-fafafa)">{{ s.FORGOT_YOUR_PASSWORD }}</span>
                        </div>
                      </div>
                      <div style="display: grid; position: relative; justify-items: start; align-items: start; align-self: stretch">
                        <div style="grid-area: 1 / 1; position: relative; z-index: 1; display: flex; min-width: 0; justify-self: stretch">
                          <div style="display: flex; flex-direction: column; align-items: flex-start; flex: 1 1 0%; min-width: 0">
                            <div style="display: block; padding-top: var(--vectis-spacing-space-4); padding-right: var(--vectis-spacing-space-40); padding-bottom: var(--vectis-spacing-space-4); padding-left: var(--vectis-spacing-space-12); background: var(--vectis-color-color-ffffff0b); border-radius: var(--vectis-corner-radius-radius-8); border: var(--vectis-border-border-1) solid var(--vectis-color-color-ffffff26); box-shadow: var(--vectis-box-shadow-shadow-677a96d2); overflow: hidden; width: 320px; height: 36px; flex-shrink: 0">
                              <div aria-hidden="true" style="display: block"></div>
                            </div>
                          </div>
                        </div>
                        <div class="vx-layer" data-vx-position="center-end" style="position: absolute; inset: 0; z-index: 2; display: flex; align-items: center; justify-content: flex-end; pointer-events: none">
                          <div class="vx-offset" style="position: relative; pointer-events: auto; display: flex">
                            <!-- TODO(event): unfilled hole, raises nothing -->
                            <button class="vectis-type-apple-system-14-medium-lh20" type="button" style="padding-top: var(--vectis-spacing-space-8); padding-right: var(--vectis-spacing-space-12); padding-bottom: var(--vectis-spacing-space-8); padding-left: var(--vectis-spacing-space-12); border-radius: var(--vectis-corner-radius-radius-8); width: 32px; flex-shrink: 0; white-space: nowrap; display: flex; flex-direction: row; align-items: center; justify-content: center; text-align: center; color: var(--vectis-color-color-fafafa); column-gap: 8px; row-gap: 8px" data-vx-hole="event">
                            </button>
                          </div>
                        </div>
                      </div>
                    </div>
                    <!-- TODO(event): unfilled hole, raises nothing -->
                    <button class="vectis-type-apple-system-14-medium-lh20" type="submit" style="padding-top: var(--vectis-spacing-space-8); padding-right: var(--vectis-spacing-space-16); padding-bottom: var(--vectis-spacing-space-8); padding-left: var(--vectis-spacing-space-16); background: var(--vectis-color-color-27a495); border-radius: var(--vectis-corner-radius-radius-8); box-shadow: var(--vectis-box-shadow-shadow-677a96d2); align-self: stretch; white-space: nowrap; display: flex; flex-direction: row; align-items: center; justify-content: center; text-align: center; color: var(--vectis-color-color-fafafa); column-gap: 8px; flex-shrink: 0; row-gap: 8px" data-vx-hole="event">
                      {{ s.LOG_IN }}
                    </button>
                  </div>
                  <div style="display: block; text-align: center; align-self: stretch">
                    <span class="vectis-type-apple-system-14-regular-lh20" style="color: var(--vectis-color-color-fafafa); display: inline">{{ s.DON_T_HAVE_AN_ACCOUNT_YET }}</span>
                    <!-- TODO(event): unfilled hole, raises nothing -->
                    <a class="vectis-type-apple-system-14-regular-lh20" style="color: var(--vectis-color-color-fafafa); display: inline; text-decoration-line: underline" href="/signup" data-vx-hole="event">{{ s.SIGN_UP }}</a>
                  </div>
                </div>
              </div>
            </div>
            <div style="display: flex; flex-direction: column; align-items: flex-start; padding-top: var(--vectis-spacing-space-16); padding-right: var(--vectis-spacing-space-24); padding-bottom: var(--vectis-spacing-space-16); padding-left: var(--vectis-spacing-space-24); align-self: stretch; border-top-color: rgba(255, 255, 255, 0.1); border-top-style: solid; border-top-width: 1px">
              <div style="display: flex; flex-direction: row; justify-content: space-between; align-items: center; column-gap: var(--vectis-spacing-space-16); row-gap: var(--vectis-spacing-space-16); align-self: stretch">
                <span class="vectis-type-apple-system-14-regular-lh20" style="color: var(--vectis-color-color-a1a1a1)">{{ s.FULL_STACK_FASTAPI_TEMPLATE_2026 }}</span>
                <div style="display: flex; flex-direction: row; align-items: center; column-gap: var(--vectis-spacing-space-16); row-gap: var(--vectis-spacing-space-16)">
                  <div style="display: flex; flex-direction: column; align-items: flex-start">
                    <svg style="width: 20px; height: 20px; flex-shrink: 0; color: var(--vectis-color-color-a1a1a1)" class="h-5 w-5" fill="currentColor" height="1em" stroke="currentColor" stroke-width="0" viewBox="0 0 496 512" width="1em" xmlns="http://www.w3.org/2000/svg"><path d="M165.9 397.4c0 2-2.3 3.6-5.2 3.6-3.3.3-5.6-1.3-5.6-3.6 0-2 2.3-3.6 5.2-3.6 3-.3 5.6 1.3 5.6 3.6zm-31.1-4.5c-.7 2 1.3 4.3 4.3 4.9 2.6 1 5.6 0 6.2-2s-1.3-4.3-4.3-5.2c-2.6-.7-5.5.3-6.2 2.3zm44.2-1.7c-2.9.7-4.9 2.6-4.6 4.9.3 2 2.9 3.3 5.9 2.6 2.9-.7 4.9-2.6 4.6-4.6-.3-1.9-3-3.2-5.9-2.9zM244.8 8C106.1 8 0 113.3 0 252c0 110.9 69.8 205.8 169.5 239.2 12.8 2.3 17.3-5.6 17.3-12.1 0-6.2-.3-40.4-.3-61.4 0 0-70 15-84.7-29.8 0 0-11.4-29.1-27.8-36.6 0 0-22.9-15.7 1.6-15.4 0 0 24.9 2 38.6 25.8 21.9 38.6 58.6 27.5 72.9 20.9 2.3-16 8.8-27.1 16-33.7-55.9-6.2-112.3-14.3-112.3-110.5 0-27.5 7.6-41.3 23.6-58.9-2.6-6.5-11.1-33.3 2.6-67.9 20.9-6.5 69 27 69 27 20-5.6 41.5-8.5 62.8-8.5s42.8 2.9 62.8 8.5c0 0 48.1-33.6 69-27 13.7 34.7 5.2 61.4 2.6 67.9 16 17.7 25.8 31.5 25.8 58.9 0 96.5-58.9 104.2-114.8 110.5 9.2 7.9 17 22.9 17 46.4 0 33.7-.3 75.4-.3 83.6 0 6.5 4.6 14.4 17.3 12.1C428.2 457.8 496 362.9 496 252 496 113.3 383.5 8 244.8 8zM97.2 352.9c-1.3 1-1 3.3.7 5.2 1.6 1.6 3.9 2.3 5.2 1 1.3-1 1-3.3-.7-5.2-1.6-1.6-3.9-2.3-5.2-1zm-10.8-8.1c-.7 1.3.3 2.9 2.3 3.9 1.6 1 3.6.7 4.3-.7.7-1.3-.3-2.9-2.3-3.9-2-.6-3.6-.3-4.3.7zm32.4 35.6c-1.6 1.3-1 4.3 1.3 6.2 2.3 2.3 5.2 2.6 6.5 1 1.3-1.3.7-4.3-1.3-6.2-2.2-2.3-5.2-2.6-6.5-1zm-11.4-14.7c-1.6 1-1.6 3.6 0 5.9 1.6 2.3 4.3 3.3 5.6 2.3 1.6-1.3 1.6-3.9 0-6.2-1.4-2.3-4-3.3-5.6-2z"></path></svg>
                  </div>
                  <div style="display: flex; flex-direction: column; align-items: flex-start">
                    <svg style="width: 20px; height: 20px; flex-shrink: 0; color: var(--vectis-color-color-a1a1a1)" class="h-5 w-5" fill="currentColor" height="1em" stroke="currentColor" stroke-width="0" viewBox="0 0 512 512" width="1em" xmlns="http://www.w3.org/2000/svg"><path d="M389.2 48h70.6L305.6 224.2 487 464H345L233.7 318.6 106.5 464H35.8L200.7 275.5 26.8 48H172.4L272.9 180.9 389.2 48zM364.4 421.8h39.1L151.1 88h-42L364.4 421.8z"></path></svg>
                  </div>
                  <div style="display: flex; flex-direction: column; align-items: flex-start">
                    <svg style="width: 20px; height: 20px; flex-shrink: 0; color: var(--vectis-color-color-a1a1a1)" class="h-5 w-5" fill="currentColor" height="1em" stroke="currentColor" stroke-width="0" viewBox="0 0 448 512" width="1em" xmlns="http://www.w3.org/2000/svg"><path d="M100.28 448H7.4V148.9h92.88zM53.79 108.1C24.09 108.1 0 83.5 0 53.8a53.79 53.79 0 0 1 107.58 0c0 29.7-24.1 54.3-53.79 54.3zM447.9 448h-92.68V302.4c0-34.7-.7-79.2-48.29-79.2-48.29 0-55.69 37.7-55.69 76.7V448h-92.78V148.9h89.08v40.8h1.3c12.4-23.5 42.69-48.3 87.88-48.3 94 0 111.28 61.9 111.28 142.3V448z"></path></svg>
                  </div>
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>
      <div style="display: block; align-self: stretch">
        <div aria-hidden="true" style="display: block"></div>
      </div>
    </div>
  </main>
</template>
