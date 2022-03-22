.PHONY: clean

TARGET = hoshinova

$(TARGET): clean
	go build -tags netgo -o $(TARGET)
	upx $(TARGET)

clean:
	rm -f $(TARGET)
