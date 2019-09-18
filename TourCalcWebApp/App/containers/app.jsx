import React from 'react';
import ReactDOM from 'react-dom';
import { BrowserRouter as Router, Route, Switch } from 'react-router-dom';
import Header from './header/header.jsx';
import TourPersons from './tour/persons.jsx';
import TourSpendings from './tour/spendings.jsx';
import TourChoose from './tour/choose.jsx';
import Auth from './auth/auth.jsx';

export default class App extends React.Component {
    render() {
        return (
            <Router>
                <div>
                    <Header />
                    <main>
                        <Switch>
                            <Route path="/spendings" component={TourSpendings} />
                            <Route path="/choose" component={TourChoose} />
                            <Route path="/auth" component={Auth} />
                            <Route path="/" component={TourPersons} />
                        </Switch>
                    </main>
                </div>
            </Router>
        );
    }
};
ReactDOM.render(
    <App />,
    document.getElementById("content")
);