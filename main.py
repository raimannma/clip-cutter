import os
import pickle
import shutil
import time
from functools import lru_cache

import numpy as np
from skimage.io import imread, imsave
from skimage.transform import resize
from skl2onnx import convert_sklearn
from skl2onnx.common.data_types import FloatTensorType
from sklearn.metrics import accuracy_score, classification_report
from sklearn.model_selection import train_test_split
from sklearn.svm import SVC
from tqdm import tqdm


def train():
    X, y = load_dataset()
    X_train, X_test, y_train, y_test = train_test_split(X, y, train_size=0.9, shuffle=True)

    classifier = SVC(verbose=True, tol=1e-6, max_iter=10000, class_weight="balanced")

    classifier.fit(X_train, y_train)

    y_prediction = classifier.predict(X_test)

    score = accuracy_score(y_test, y_prediction)
    report = classification_report(y_test, y_prediction)
    print(report)

    print(f"{score * 100:.2f}% of samples were correctly classified")

    pickle.dump(classifier, open("./model.p", "wb"))


def preprocess_img(img_path):
    img = imread(img_path)
    if img.shape == (1080, 1920, 3):
        img = img[750:950, 860:1060]
        imsave(img_path, img)
    img = resize(img, (50, 50))
    if (img > 1).any() or (img < 0).any():
        img = img / 255.0
    return img.flatten()


@lru_cache()
def load_dataset():
    input_dir = "kill-data"
    categories = ["no_kill", "kill"]
    data = []
    labels = []
    for category_idx, category in enumerate(categories):
        files = os.listdir(os.path.join(input_dir, category))
        for file in tqdm(files, desc=category):
            img_path = os.path.join(input_dir, category, file)
            img = preprocess_img(img_path)
            if img is None:
                continue
            data.append(img)
            labels.append(category_idx)
    return np.asarray(data), np.asarray(labels)


def benchmark():
    X, y = load_dataset()
    model = pickle.load(open("./model.p", "rb"))
    times = []
    # warmup
    for img in X[:10]:
        model.predict([img])
    for img in tqdm(X[:1000], desc="Benchmarking"):
        start = time.time()
        model.predict([img])
        end = time.time()
        times.append(end - start)

    print(f"Average inference time: {np.mean(times) * 1000:.2f}ms")


def infer():
    model = pickle.load(open("./model.p", "rb"))

    # base_dir = "kill-data/kill"
    # for file in tqdm(os.listdir(base_dir)):
    #     img = preprocess_img(os.path.join(base_dir, file))
    #     prediction = model.predict([img])
    #     if prediction != 1:
    #         shutil.move(os.path.join(base_dir, file), os.path.join("kill-data/wrong", file))
    #         print(f"Wrong prediction for {file}")

    base_dir = "kill-data/no_kill"
    for file in tqdm(os.listdir(base_dir)):
        img = preprocess_img(os.path.join(base_dir, file))
        prediction = model.predict([img])
        if prediction != 0:
            shutil.move(os.path.join(base_dir, file), os.path.join("kill-data/wrong", file))
            print(f"Wrong prediction for {file}")


def export():
    model = pickle.load(open("./model.p", "rb"))
    initial_type = [("float_input", FloatTensorType([1, 50 * 50 * 3]))]
    onnx = convert_sklearn(model, initial_types=initial_type, target_opset=18, model_optim=True)
    sequence_outputs = (o for o in onnx.graph.output if o.type.WhichOneof("value") == "sequence_type")
    for o in sequence_outputs:
        onnx.graph.output.remove(o)
    with open("model.onnx", "wb") as f:
        f.write(onnx.SerializeToString())


if __name__ == "__main__":
    train()
    benchmark()
    # infer()
    export()
