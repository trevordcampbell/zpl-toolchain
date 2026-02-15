/**
 * Forward-compatible seam for future renderer-backed live preview.
 *
 * MVP uses a no-op implementation, but callers can depend on this interface
 * now so preview integration is additive later.
 */
export interface RendererBridge {
  renderLabelPreview(_zpl: string): Promise<undefined>;
}

export class NoopRendererBridge implements RendererBridge {
  async renderLabelPreview(_zpl: string): Promise<undefined> {
    return undefined;
  }
}
