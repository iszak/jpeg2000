# JPXML
Encoding of JP2 and JPC into ISO 16444 Part-14 XML representation. This is
mostly used for debugging purposes.

### Running
````
cargo run jpxml ./samples/zoo1.jp2 > zoo1.xml
````

### Testing
As there's no test harness that validates the XML generated against the XSD
it must be done manually after generating the XML.

```
xmllint --schema ./jpxml/part-1-image.xsd zoo1.xml
```

