import { HandshakeRequest } from '../cdk/handshake/schema.ts';

const handshake = {
    tag: HandshakeRequest.Tag.Handshake,
    service: 1,
    pk: new Uint8Array(96).fill(1),
    pop: new Uint8Array(48).fill(2),
}

export function testEncode() {
    const _encoded = HandshakeRequest.encode(handshake as HandshakeRequest.Frame);
    const encoded = new Uint8Array(_encoded);

    
    return Array.from(encoded)
}

export function testDecode(arr: Array<number>) {   
    const input = new Uint8Array(arr);
    const decoded = HandshakeRequest.decode(input.buffer);

    const _re_encoded = HandshakeRequest.encode(decoded as HandshakeRequest.Frame);
    const re_encoded = new Uint8Array(_re_encoded);

    // you cant compare two uint8arrays directly lmfao
    return re_encoded.length === input.length && Array.from(re_encoded).every((v, i) => v === input[i]);
}