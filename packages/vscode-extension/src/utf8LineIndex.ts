import * as vscode from "vscode";

/**
 * Maps UTF-8 byte offsets (from Rust spans) to VS Code positions.
 */
export class Utf8LineIndex {
  private readonly text: string;
  private readonly lineStartByteOffsets: number[];
  private readonly lineStartUtf16Offsets: number[];

  public constructor(text: string) {
    this.text = text;
    this.lineStartByteOffsets = [0];
    this.lineStartUtf16Offsets = [0];

    let byteOffset = 0;
    let utf16Offset = 0;

    for (const ch of text) {
      const charByteLength = Buffer.byteLength(ch, "utf8");
      const charUtf16Length = ch.length;

      byteOffset += charByteLength;
      utf16Offset += charUtf16Length;

      if (ch === "\n") {
        this.lineStartByteOffsets.push(byteOffset);
        this.lineStartUtf16Offsets.push(utf16Offset);
      }
    }
  }

  public positionAtByteOffset(inputByteOffset: number): vscode.Position {
    const maxByteOffset = Buffer.byteLength(this.text, "utf8");
    const byteOffset = Math.max(0, Math.min(inputByteOffset, maxByteOffset));

    const line = this.findLineForByteOffset(byteOffset);
    const lineStartByte = this.lineStartByteOffsets[line] ?? 0;
    const lineStartUtf16 = this.lineStartUtf16Offsets[line] ?? 0;
    const targetBytesIntoLine = byteOffset - lineStartByte;

    let consumedBytes = 0;
    let consumedUtf16 = 0;

    // Iterate from the line start to preserve correctness with multibyte chars.
    for (let i = lineStartUtf16; i < this.text.length; ) {
      const cp = this.text.codePointAt(i);
      if (cp === undefined) {
        break;
      }
      const ch = String.fromCodePoint(cp);
      const chByteLength = Buffer.byteLength(ch, "utf8");
      const chUtf16Length = ch.length;

      if (consumedBytes + chByteLength > targetBytesIntoLine || ch === "\n") {
        break;
      }

      consumedBytes += chByteLength;
      consumedUtf16 += chUtf16Length;
      i += chUtf16Length;
    }

    return new vscode.Position(line, consumedUtf16);
  }

  private findLineForByteOffset(byteOffset: number): number {
    let low = 0;
    let high = this.lineStartByteOffsets.length - 1;
    let result = 0;

    while (low <= high) {
      const mid = (low + high) >> 1;
      const lineStart = this.lineStartByteOffsets[mid] ?? 0;
      if (lineStart <= byteOffset) {
        result = mid;
        low = mid + 1;
      } else {
        high = mid - 1;
      }
    }

    return result;
  }
}
