ldr q0, [{ptr}]
movi v1.16b, #37
cmeq v0.16b, v0.16b, v1.16b
umaxv b0, v0.16b
umov {has_match:w}, v0.b[0]
