<script setup lang="ts">
import { ref, onMounted } from "vue";
import Icon from "./Icon.vue";
defineProps<{ title: string; wide?: boolean; busy?: boolean }>();
const emit = defineEmits<{ close: [] }>();
const dialog = ref<HTMLDialogElement>();
onMounted(() => dialog.value?.showModal());
</script>
<template>
  <dialog
    ref="dialog"
    class="app-dialog cl-dialog"
    :class="{ 'app-dialog--wide': wide }"
    aria-labelledby="dialog-title"
    @cancel.prevent="!busy && emit('close')"
    @click="
      (e) => {
        if (e.target === dialog && !busy) emit('close');
      }
    "
  >
    <div class="cl-dialog__head">
      <h2 id="dialog-title" class="cl-dialog__title">{{ title }}</h2>
      <button
        class="cl-btn cl-btn--ghost cl-btn--icon"
        aria-label="关闭对话框"
        :disabled="busy"
        @click="emit('close')"
      >
        <Icon name="close" />
      </button>
    </div>
    <div class="cl-dialog__body"><slot /></div>
    <div v-if="$slots.footer" class="cl-dialog__foot">
      <slot name="footer" />
    </div>
  </dialog>
</template>
