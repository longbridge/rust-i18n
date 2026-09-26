vmovdqu {data}, [{ptr}]
vpcmpeqb {data}, {data}, {needle}
vpmovmskb {mask:e}, {data}
